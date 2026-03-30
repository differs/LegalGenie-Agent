use crate::config::AppConfig;
use anyhow::{anyhow, Context};
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    Pdf,
    Image,
    Audio,
    Text,
    Excel,
    Docx,
    Doc,
    Unknown,
}

impl FileKind {
    fn as_str(self) -> &'static str {
        match self {
            FileKind::Pdf => "pdf",
            FileKind::Image => "image",
            FileKind::Audio => "audio",
            FileKind::Text => "text",
            FileKind::Excel => "excel",
            FileKind::Docx => "docx",
            FileKind::Doc => "doc",
            FileKind::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Serialize)]
struct ParsedArtifact<'a> {
    version: u32,
    evidence_id: &'a str,
    case_id: &'a str,
    original_name: &'a str,
    file_type: &'a str,
    kind: &'a str,
    parsed_at: String,
    page_count: Option<i64>,
    duration: Option<i64>,
    parsed_text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<&'a serde_json::Value>,
}

#[derive(Debug)]
struct ParseOutput {
    parsed_text: String,
    page_count: Option<i64>,
    duration: Option<i64>,
    extra: Option<serde_json::Value>,
}

#[derive(Debug)]
struct FileChunk {
    chunk_index: i64,
    page_number: i64,
    segment_number: i64,
    chunk_kind: String,
    display_label: String,
    source_text: String,
    anchor_json: String,
}

pub async fn enqueue_parse(
    pool: SqlitePool,
    config: AppConfig,
    file_id: String,
    force: bool,
) -> anyhow::Result<bool> {
    let started = mark_processing(&pool, &file_id, force).await?;
    if !started {
        return Ok(false);
    }

    tokio::spawn(async move {
        if let Err(e) = parse_and_update(&pool, &config, &file_id).await {
            tracing::error!(file_id = %file_id, error = %e, "parse failed");
            let _ = mark_failed(&pool, &file_id, &e.to_string()).await;
        }
    });

    Ok(true)
}

async fn mark_processing(pool: &SqlitePool, file_id: &str, force: bool) -> anyhow::Result<bool> {
    let sql = if force {
        "UPDATE evidence_files SET parse_status = 'processing', parse_error = NULL WHERE id = ?1 AND status != 'deleted'"
    } else {
        "UPDATE evidence_files SET parse_status = 'processing', parse_error = NULL WHERE id = ?1 AND status != 'deleted' AND parse_status != 'processing'"
    };

    let res = sqlx::query(sql)
        .bind(file_id)
        .execute(pool)
        .await
        .context("update parse_status=processing")?;

    Ok(res.rows_affected() > 0)
}

async fn mark_failed(pool: &SqlitePool, file_id: &str, err: &str) -> anyhow::Result<()> {
    let err = truncate_string(err, 2000);
    sqlx::query(
        "UPDATE evidence_files SET parse_status = 'failed', parse_error = ?1, parsed_at = CURRENT_TIMESTAMP WHERE id = ?2",
    )
    .bind(err)
    .bind(file_id)
    .execute(pool)
    .await
    .context("update parse_status=failed")?;
    Ok(())
}

async fn persist_chunks_and_mark_done(
    pool: &SqlitePool,
    row: &EvidenceFileToParse,
    out: ParseOutput,
    mut chunks: Vec<FileChunk>,
) -> anyhow::Result<()> {
    chunks.sort_by_key(|c| c.chunk_index);
    let mut tx = pool.begin().await.context("begin parse persistence tx")?;

    sqlx::query("DELETE FROM evidence_file_chunks WHERE evidence_id = ?1")
        .bind(&row.id)
        .execute(&mut *tx)
        .await
        .context("delete old file chunks")?;

    for chunk in &chunks {
        let char_count = chunk.source_text.chars().count() as i64;
        let token_estimate = if char_count <= 0 {
            0
        } else {
            ((char_count + 3) / 4).max(1)
        };
        sqlx::query(
            r#"
            INSERT INTO evidence_file_chunks (
                id,
                evidence_id,
                case_id,
                chunk_index,
                page_number,
                segment_number,
                chunk_kind,
                display_label,
                source_text,
                char_count,
                token_estimate,
                anchor_json,
                source_text_hash,
                translation_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'pending')
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&row.id)
        .bind(&row.case_id)
        .bind(chunk.chunk_index)
        .bind(chunk.page_number)
        .bind(chunk.segment_number)
        .bind(&chunk.chunk_kind)
        .bind(&chunk.display_label)
        .bind(&chunk.source_text)
        .bind(char_count)
        .bind(token_estimate)
        .bind(&chunk.anchor_json)
        .bind(source_text_hash(&chunk.source_text))
        .execute(&mut *tx)
        .await
        .context("insert evidence_file_chunk")?;
    }

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET parsed_text = ?1,
            page_count = ?2,
            duration = ?3,
            parse_status = 'done',
            parse_error = NULL,
            parsed_at = CURRENT_TIMESTAMP,
            translation_status = 'pending',
            translation_error = NULL,
            translated_chunk_count = 0,
            failed_chunk_count = 0,
            source_language = NULL,
            translation_model = NULL,
            translation_provider = NULL,
            chunk_count = ?4
        WHERE id = ?5
        "#,
    )
    .bind(out.parsed_text)
    .bind(out.page_count)
    .bind(out.duration)
    .bind(chunks.len() as i64)
    .bind(&row.id)
    .execute(&mut *tx)
    .await
    .context("update parse_status=done with chunk reset")?;

    tx.commit().await.context("commit parse persistence tx")?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct EvidenceFileToParse {
    id: String,
    case_id: String,
    original_name: String,
    file_type: String,
    storage_path: String,
}

async fn parse_and_update(
    pool: &SqlitePool,
    config: &AppConfig,
    file_id: &str,
) -> anyhow::Result<()> {
    let row: Option<EvidenceFileToParse> = sqlx::query_as(
        r#"
        SELECT id, case_id, original_name, file_type, storage_path
        FROM evidence_files
        WHERE id = ?1 AND status != 'deleted'
        LIMIT 1
        "#,
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await
    .context("fetch evidence_files row")?;

    let Some(row) = row else {
        return Ok(());
    };

    let full_path = PathBuf::from(&config.storage_path).join(&row.storage_path);
    let kind = detect_kind(&row.original_name, &row.file_type);

    let mut out = match kind {
        FileKind::Pdf => parse_pdf(&full_path).await?,
        FileKind::Image => parse_image(&full_path, config).await?,
        FileKind::Audio => parse_audio(&full_path, config, &row.id).await?,
        FileKind::Text => parse_text(&full_path).await?,
        FileKind::Excel => parse_excel(&full_path).await?,
        FileKind::Docx => parse_docx(&full_path).await?,
        FileKind::Doc => parse_doc(&full_path).await?,
        FileKind::Unknown => {
            return Err(anyhow!(
                "unsupported file type: original_name={} file_type={}",
                row.original_name,
                row.file_type
            ));
        }
    };

    // Guard runaway output (and keep json/db consistent).
    out.parsed_text = truncate_string(&out.parsed_text, 50_000_000);
    write_parsed_artifact(config, &row, kind, &out).await?;

    let chunks = derive_chunks(kind, &out, translation_chunk_size_limit());
    persist_chunks_and_mark_done(pool, &row, out, chunks).await?;
    Ok(())
}

fn derive_chunks(kind: FileKind, out: &ParseOutput, chunk_size_limit: usize) -> Vec<FileChunk> {
    let limit = chunk_size_limit.max(1);
    match kind {
        FileKind::Pdf => derive_pdf_chunks(&out.parsed_text, limit),
        FileKind::Excel => derive_excel_chunks(&out.parsed_text, limit),
        FileKind::Audio => derive_audio_chunks(&out.parsed_text, out.duration, limit),
        FileKind::Text | FileKind::Docx | FileKind::Image | FileKind::Doc | FileKind::Unknown => {
            derive_text_chunks(&out.parsed_text, limit)
        }
    }
}

fn derive_text_chunks(text: &str, chunk_size_limit: usize) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    for (idx, piece) in split_paragraphs_and_limit(text, chunk_size_limit)
        .into_iter()
        .enumerate()
    {
        let segment_number = (idx + 1) as i64;
        let display_label = format!("第 {segment_number} 段");
        let anchor_json = serde_json::json!({
            "locator_type": "segment",
            "display_label": display_label,
            "segment_number": segment_number,
        });
        push_chunk(&mut chunks, 0, segment_number, "text", anchor_json, piece);
    }
    chunks
}

fn derive_pdf_chunks(text: &str, chunk_size_limit: usize) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let raw_pages = split_pdf_pages(text);

    for (page_idx, page_text) in raw_pages.iter().enumerate() {
        let page_number = (page_idx + 1) as i64;
        let pieces = split_paragraphs_and_limit(page_text, chunk_size_limit);
        if pieces.is_empty() {
            continue;
        }

        let total = pieces.len() as i64;
        for (sub_idx, piece) in pieces.into_iter().enumerate() {
            let seg = (sub_idx + 1) as i64;
            let display_label = if total <= 1 {
                format!("第 {page_number} 页")
            } else {
                format!("第 {page_number} 页({seg}/{total})")
            };
            let anchor_json = serde_json::json!({
                "locator_type": "page",
                "display_label": display_label,
                "page_number": page_number,
            });
            push_chunk(
                &mut chunks,
                page_number,
                seg,
                "pdf_page",
                anchor_json,
                piece,
            );
        }
    }

    chunks
}

fn derive_excel_chunks(text: &str, chunk_size_limit: usize) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let sheets = split_excel_sheets(text);
    let mut segment_number = 1i64;

    for (sheet_name, sheet_text) in sheets {
        let pieces = split_paragraphs_and_limit(&sheet_text, chunk_size_limit);
        if pieces.is_empty() {
            continue;
        }
        let total = pieces.len() as i64;
        for (sub_idx, piece) in pieces.into_iter().enumerate() {
            let section = (sub_idx + 1) as i64;
            let display_label = if total <= 1 {
                format!("工作表 {}", sheet_name)
            } else {
                format!("工作表 {}({}/{})", sheet_name, section, total)
            };
            let anchor_json = serde_json::json!({
                "locator_type": "sheet",
                "display_label": display_label,
                "sheet_name": sheet_name,
                "cell_range": format!("synthetic:{section}/{total}"),
            });
            push_chunk(&mut chunks, 0, segment_number, "sheet", anchor_json, piece);
            segment_number += 1;
        }
    }

    chunks
}

fn derive_audio_chunks(
    text: &str,
    duration: Option<i64>,
    chunk_size_limit: usize,
) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let mut pieces = split_paragraphs_and_limit(text, chunk_size_limit);
    if pieces.is_empty() {
        pieces.push("[audio transcript unavailable]".to_string());
    }

    let total = pieces.len() as i64;
    let d = duration.unwrap_or(0).max(0);
    for (idx, piece) in pieces.into_iter().enumerate() {
        let segment_number = (idx + 1) as i64;
        let (start_second, end_second) = if d > 0 {
            let start = (d * idx as i64) / total;
            let end = (d * (idx as i64 + 1)) / total;
            (start, end.max(start))
        } else {
            let start = idx as i64 * 30;
            (start, start + 30)
        };
        let display_label = format!("第 {segment_number} 段");
        let anchor_json = serde_json::json!({
            "locator_type": "time_range_synthetic",
            "display_label": display_label,
            "segment_number": segment_number,
            "start_second": start_second,
            "end_second": end_second,
            "synthetic": true,
        });
        push_chunk(
            &mut chunks,
            0,
            segment_number,
            "audio_segment",
            anchor_json,
            piece,
        );
    }

    chunks
}

fn push_chunk(
    chunks: &mut Vec<FileChunk>,
    page_number: i64,
    segment_number: i64,
    chunk_kind: &str,
    anchor_json: serde_json::Value,
    source_text: String,
) {
    let source_text = source_text.trim().to_string();
    if source_text.is_empty() {
        return;
    }

    let display_label = anchor_json
        .get("display_label")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    chunks.push(FileChunk {
        chunk_index: chunks.len() as i64,
        page_number,
        segment_number,
        chunk_kind: chunk_kind.to_string(),
        display_label,
        source_text,
        anchor_json: anchor_json.to_string(),
    });
}

fn split_pdf_pages(text: &str) -> Vec<String> {
    let normalized = normalize_newlines(text);
    if normalized.contains('\u{000c}') {
        normalized
            .split('\u{000c}')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect()
    } else {
        let trimmed = normalized.trim();
        if trimmed.is_empty() {
            Vec::new()
        } else {
            vec![trimmed.to_string()]
        }
    }
}

fn split_excel_sheets(text: &str) -> Vec<(String, String)> {
    let normalized = normalize_newlines(text);
    let mut sheets: Vec<(String, String)> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in normalized.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Sheet:") {
            if let Some(name) = current_name.take() {
                let sheet_text = current_lines.join("\n").trim().to_string();
                if !sheet_text.is_empty() {
                    sheets.push((name, sheet_text));
                }
                current_lines.clear();
            }
            let name = rest.trim();
            current_name = Some(if name.is_empty() {
                "Unknown".to_string()
            } else {
                name.to_string()
            });
            continue;
        }

        if !trimmed.is_empty() {
            current_lines.push(trimmed.to_string());
        }
    }

    if let Some(name) = current_name.take() {
        let sheet_text = current_lines.join("\n").trim().to_string();
        if !sheet_text.is_empty() {
            sheets.push((name, sheet_text));
        }
    }

    if sheets.is_empty() {
        let trimmed = normalized.trim();
        if !trimmed.is_empty() {
            sheets.push(("Sheet1".to_string(), trimmed.to_string()));
        }
    }

    sheets
}

fn split_paragraphs_and_limit(text: &str, chunk_size_limit: usize) -> Vec<String> {
    let paragraphs = split_paragraphs(text);
    let mut out = Vec::new();
    for paragraph in paragraphs {
        out.extend(split_with_limit(&paragraph, chunk_size_limit));
    }
    out
}

fn split_paragraphs(text: &str) -> Vec<String> {
    let normalized = normalize_newlines(text);
    let mut paragraphs = Vec::new();
    let mut current = Vec::new();

    for line in normalized.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
            continue;
        }
        current.push(trimmed.to_string());
    }

    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }

    if paragraphs.is_empty() && !normalized.trim().is_empty() {
        paragraphs.push(normalized.trim().to_string());
    }

    paragraphs
}

fn split_with_limit(text: &str, chunk_size_limit: usize) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.chars().count() <= chunk_size_limit {
        return vec![trimmed.to_string()];
    }

    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0usize;

    for ch in trimmed.chars() {
        current.push(ch);
        current_chars += 1;
        if current_chars >= chunk_size_limit {
            let piece = current.trim();
            if !piece.is_empty() {
                out.push(piece.to_string());
            }
            current.clear();
            current_chars = 0;
        }
    }

    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }

    if out.is_empty() {
        out.push(trimmed.to_string());
    }
    out
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn translation_chunk_size_limit() -> usize {
    std::env::var("TRANSLATION_CHUNK_SIZE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(2_000)
}

fn detect_kind(original_name: &str, file_type: &str) -> FileKind {
    let ft = file_type.to_ascii_lowercase();
    if ft.starts_with("application/pdf") {
        return FileKind::Pdf;
    }
    if ft.starts_with("image/") {
        return FileKind::Image;
    }
    if ft.starts_with("audio/") {
        return FileKind::Audio;
    }
    if ft.starts_with("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        || ft.starts_with("application/vnd.ms-excel")
    {
        return FileKind::Excel;
    }
    if ft.starts_with("application/vnd.openxmlformats-officedocument.wordprocessingml.document") {
        return FileKind::Docx;
    }
    if ft.starts_with("application/msword") {
        return FileKind::Doc;
    }
    if ft.starts_with("text/") || ft.starts_with("application/json") {
        return FileKind::Text;
    }

    let ext = Path::new(original_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "pdf" => FileKind::Pdf,
        "png" | "jpg" | "jpeg" | "bmp" | "tif" | "tiff" => FileKind::Image,
        "mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" => FileKind::Audio,
        "txt" | "md" | "csv" | "json" => FileKind::Text,
        "xlsx" | "xls" => FileKind::Excel,
        "docx" => FileKind::Docx,
        "doc" => FileKind::Doc,
        _ => FileKind::Unknown,
    }
}

async fn parse_text(path: &Path) -> anyhow::Result<ParseOutput> {
    let bytes = tokio::fs::read(path).await.context("read file")?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    Ok(ParseOutput {
        parsed_text: text,
        page_count: None,
        duration: None,
        extra: None,
    })
}

async fn parse_pdf(path: &Path) -> anyhow::Result<ParseOutput> {
    let text = run_cmd_capture_stdout("pdftotext", &["-layout", path_str(path)?, "-"]).await?;
    let page_count = match pdf_page_count(path).await {
        Ok(n) => Some(n),
        Err(_) => None,
    };

    Ok(ParseOutput {
        parsed_text: text,
        page_count,
        duration: None,
        extra: None,
    })
}

async fn pdf_page_count(path: &Path) -> anyhow::Result<i64> {
    let out = run_cmd_capture_stdout("pdfinfo", &[path_str(path)?]).await?;
    for line in out.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("Pages:") {
            let n = rest.trim().parse::<i64>().context("parse pages")?;
            return Ok(n);
        }
    }
    Err(anyhow!("Pages not found in pdfinfo output"))
}

async fn parse_image(path: &Path, config: &AppConfig) -> anyhow::Result<ParseOutput> {
    let tessdata_dir = Path::new(&config.tessdata_dir);
    let has_chi = tokio::fs::try_exists(tessdata_dir.join("chi_sim.traineddata"))
        .await
        .unwrap_or(false);
    let lang = if has_chi { "chi_sim+eng" } else { "eng" };

    let text = run_cmd_capture_stdout(
        "tesseract",
        &[
            path_str(path)?,
            "stdout",
            "-l",
            lang,
            "--tessdata-dir",
            &config.tessdata_dir,
        ],
    )
    .await?;

    Ok(ParseOutput {
        parsed_text: text,
        page_count: None,
        duration: None,
        extra: None,
    })
}

async fn parse_audio(
    path: &Path,
    config: &AppConfig,
    file_id: &str,
) -> anyhow::Result<ParseOutput> {
    let duration = audio_duration_seconds(path).await.ok().map(|d| d as i64);
    let wav_path = PathBuf::from(&config.temp_path).join(format!("{file_id}_16k.wav"));

    convert_audio_to_wav_16k(path, &wav_path).await?;

    let model_path = config.whisper_model_path.clone();
    let asr_language = config.asr_language.clone();
    let threads = config.asr_threads;
    let wav_path2 = wav_path.clone();

    let transcript = tokio::task::spawn_blocking(move || {
        transcribe_with_whisper_rs(&model_path, &wav_path2, &asr_language, threads)
    })
    .await
    .context("join asr task")??;

    let _ = tokio::fs::remove_file(&wav_path).await;

    Ok(ParseOutput {
        parsed_text: transcript,
        page_count: None,
        duration,
        extra: Some(serde_json::json!({
            "duration_seconds": duration,
            "segments": null
        })),
    })
}

async fn parse_excel(path: &Path) -> anyhow::Result<ParseOutput> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || parse_excel_sync(&path))
        .await
        .context("join excel parse task")?
}

fn parse_excel_sync(path: &Path) -> anyhow::Result<ParseOutput> {
    use calamine::{open_workbook_auto, Data, Reader};

    const MAX_CELLS: usize = 200_000;
    const MAX_TEXT_BYTES: usize = 5_000_000;

    let mut workbook =
        open_workbook_auto(path).with_context(|| format!("open excel: {}", path.display()))?;
    let sheet_names = workbook.sheet_names();
    let mut text = String::new();
    let mut cell_count: usize = 0;
    let mut truncated = false;

    for sheet in &sheet_names {
        if cell_count >= MAX_CELLS || text.len() >= MAX_TEXT_BYTES {
            truncated = true;
            break;
        }

        let range = match workbook.worksheet_range(sheet) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %format!("{e:?}"), sheet = sheet, "skip unreadable sheet");
                continue;
            }
        };

        text.push_str("Sheet: ");
        text.push_str(sheet);
        text.push('\n');

        for row in range.rows() {
            for cell in row {
                if cell_count >= MAX_CELLS || text.len() >= MAX_TEXT_BYTES {
                    truncated = true;
                    break;
                }

                let s = match cell {
                    Data::Empty => String::new(),
                    Data::String(s) => s.trim().to_string(),
                    Data::Float(f) => {
                        if f.fract() == 0.0 {
                            format!("{}", *f as i64)
                        } else {
                            f.to_string()
                        }
                    }
                    Data::Int(i) => i.to_string(),
                    Data::Bool(b) => b.to_string(),
                    other => other.to_string(),
                };

                if !s.is_empty() {
                    text.push_str(&s);
                    text.push(' ');
                }
                cell_count = cell_count.saturating_add(1);
            }

            if truncated {
                break;
            }
            text.push('\n');
        }
        text.push('\n');
    }

    Ok(ParseOutput {
        parsed_text: text.trim().to_string(),
        page_count: None,
        duration: None,
        extra: Some(serde_json::json!({
            "sheet_names": sheet_names,
            "cell_count": cell_count,
            "truncated": truncated,
        })),
    })
}

async fn parse_docx(path: &Path) -> anyhow::Result<ParseOutput> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || parse_docx_sync(&path))
        .await
        .context("join docx parse task")?
}

fn parse_docx_sync(path: &Path) -> anyhow::Result<ParseOutput> {
    use quick_xml::escape;
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::io::Read;

    let f = std::fs::File::open(path).with_context(|| format!("open docx: {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(f).context("open docx zip")?;

    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .context("missing word/document.xml")?
        .read_to_string(&mut xml)
        .context("read document.xml")?;

    let mut reader = Reader::from_str(&xml);
    // Keep whitespace as-is, docx text often depends on it.
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut out = String::new();
    let mut in_text = false;
    let mut paragraph_count: i64 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local_name(qname.as_ref());
                if name == b"t" {
                    in_text = true;
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local_name(qname.as_ref());
                if name == b"t" {
                    in_text = false;
                } else if name == b"p" {
                    out.push('\n');
                    paragraph_count = paragraph_count.saturating_add(1);
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local_name(qname.as_ref());
                if name == b"tab" {
                    out.push('\t');
                } else if name == b"br" {
                    out.push('\n');
                }
            }
            Ok(Event::Text(e)) => {
                if in_text {
                    let decoded = e.xml_content().context("decode docx text")?;
                    let unescaped =
                        escape::unescape(decoded.as_ref()).context("unescape docx text")?;
                    out.push_str(unescaped.as_ref());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("docx xml parse failed: {e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(ParseOutput {
        parsed_text: out.trim().to_string(),
        page_count: None,
        duration: None,
        extra: Some(serde_json::json!({
            "paragraph_count": paragraph_count,
        })),
    })
}

fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().rposition(|b| *b == b':') {
        Some(idx) => &raw[idx + 1..],
        None => raw,
    }
}

async fn parse_doc(_path: &Path) -> anyhow::Result<ParseOutput> {
    Err(anyhow!(
        "legacy .doc is not supported yet; please convert to .docx"
    ))
}

async fn write_parsed_artifact(
    config: &AppConfig,
    row: &EvidenceFileToParse,
    kind: FileKind,
    out: &ParseOutput,
) -> anyhow::Result<()> {
    let rel = format!("parsed/{}/{}.json", row.case_id, row.id);
    let full_path = PathBuf::from(&config.storage_path).join(&rel);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create parsed dir: {}", parent.display()))?;
    }

    let payload = ParsedArtifact {
        version: 1,
        evidence_id: &row.id,
        case_id: &row.case_id,
        original_name: &row.original_name,
        file_type: &row.file_type,
        kind: kind.as_str(),
        parsed_at: Utc::now().to_rfc3339(),
        page_count: out.page_count,
        duration: out.duration,
        parsed_text: &out.parsed_text,
        extra: out.extra.as_ref(),
    };

    let bytes = serde_json::to_vec(&payload).context("serialize parsed json")?;
    tokio::fs::write(&full_path, bytes)
        .await
        .with_context(|| format!("write parsed json: {}", full_path.display()))?;

    Ok(())
}

async fn audio_duration_seconds(path: &Path) -> anyhow::Result<f64> {
    let out = run_cmd_capture_stdout(
        "ffprobe",
        &[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            path_str(path)?,
        ],
    )
    .await?;

    let s = out.trim();
    if s.is_empty() {
        return Err(anyhow!("ffprobe returned empty duration"));
    }
    Ok(s.parse::<f64>().context("parse duration")?)
}

async fn convert_audio_to_wav_16k(input: &Path, output_wav: &Path) -> anyhow::Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(path_str(input)?)
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-f")
        .arg("wav")
        .arg(path_str(output_wav)?)
        .output()
        .await
        .context("run ffmpeg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg failed: {}", stderr.trim()));
    }
    Ok(())
}

fn transcribe_with_whisper_rs(
    model_path: &str,
    wav_path: &Path,
    language: &str,
    threads: u16,
) -> anyhow::Result<String> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    if !std::path::Path::new(model_path).exists() {
        return Err(anyhow!(
            "whisper model not found at {} (set WHISPER_MODEL_PATH)",
            model_path
        ));
    }

    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| anyhow!("failed to load whisper model: {e}"))?;
    let mut state = ctx
        .create_state()
        .map_err(|_| anyhow!("failed to create whisper state"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
    params.set_n_threads(threads as i32);
    params.set_translate(false);
    if !language.trim().is_empty() && language.trim() != "auto" {
        params.set_language(Some(language));
    }
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    let reader = hound::WavReader::open(wav_path).context("open wav")?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 {
        return Err(anyhow!("wav sample rate must be 16000Hz"));
    }

    let samples: Vec<i16> = reader
        .into_samples::<i16>()
        .map(|x| x.map_err(|e| anyhow!("invalid wav sample: {e}")))
        .collect::<Result<Vec<_>, _>>()?;

    let mut audio = vec![0.0f32; samples.len()];
    whisper_rs::convert_integer_to_float_audio(&samples, &mut audio)
        .map_err(|_| anyhow!("convert audio failed"))?;

    let audio = match spec.channels {
        1 => audio,
        2 => {
            let mut out = vec![0.0f32; audio.len() / 2];
            whisper_rs::convert_stereo_to_mono_audio(&audio, &mut out)
                .map_err(|_| anyhow!("stereo to mono failed"))?;
            out
        }
        n => return Err(anyhow!("unsupported wav channels: {n}")),
    };

    state
        .full(params, &audio)
        .map_err(|_| anyhow!("whisper inference failed"))?;

    let mut out = String::new();
    for segment in state.as_iter() {
        let s = segment.to_string();
        if !s.trim().is_empty() {
            out.push_str(s.trim());
            out.push('\n');
        }
    }

    Ok(out.trim().to_string())
}

async fn run_cmd_capture_stdout(cmd: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .await
        .with_context(|| format!("run {}", cmd))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("{} failed: {}", cmd, stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn path_str(path: &Path) -> anyhow::Result<&str> {
    path.to_str().ok_or_else(|| anyhow!("non-utf8 path"))
}

fn source_text_hash(text: &str) -> String {
    // Deterministic FNV-1a 64-bit hash for chunk dedupe/indexing.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn truncate_string(s: impl AsRef<str>, max_chars: usize) -> String {
    s.as_ref().chars().take(max_chars).collect()
}
