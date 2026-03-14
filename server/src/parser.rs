use crate::config::AppConfig;
use anyhow::{anyhow, Context};
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tokio::process::Command;

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

async fn mark_done(pool: &SqlitePool, file_id: &str, out: ParseOutput) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE evidence_files
        SET parsed_text = ?1,
            page_count = ?2,
            duration = ?3,
            parse_status = 'done',
            parse_error = NULL,
            parsed_at = CURRENT_TIMESTAMP
        WHERE id = ?4
        "#,
    )
    .bind(out.parsed_text)
    .bind(out.page_count)
    .bind(out.duration)
    .bind(file_id)
    .execute(pool)
    .await
    .context("update parse_status=done")?;
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

    mark_done(pool, &row.id, out).await?;
    Ok(())
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

fn truncate_string(s: impl AsRef<str>, max_chars: usize) -> String {
    s.as_ref().chars().take(max_chars).collect()
}
