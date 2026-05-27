use crate::errors::{AppError, AppResult};
use std::path::Path;

const SUPPORTED_MIME_TYPES: &[&str] = &[
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "image/jpeg",
    "image/png",
    "audio/mpeg",
    "audio/wav",
    "text/plain",
    "application/json",
];

pub fn validate_upload(
    header_bytes: &[u8],
    filename: &str,
    allowed_file_types: &[String],
) -> AppResult<String> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if ext.is_empty() {
        return Err(AppError::bad_request_code(420103, "invalid file extension"));
    }

    if !allowed_file_types.is_empty() && !allowed_file_types.iter().any(|t| t == &ext) {
        return Err(AppError::bad_request_code(420103, "unsupported file type"));
    }

    let guessed = mime_guess::from_ext(&ext)
        .first_raw()
        .ok_or_else(|| AppError::bad_request_code(420103, "unknown file type"))?;

    if !SUPPORTED_MIME_TYPES.contains(&guessed) {
        return Err(AppError::bad_request_code(420103, "unsupported file type"));
    }

    let detected = infer::get(header_bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");

    if !mime_compatible(guessed, detected) {
        return Err(AppError::bad_request_code(420103, "file type mismatch"));
    }

    Ok(guessed.to_string())
}

fn mime_compatible(guessed: &str, detected: &str) -> bool {
    if guessed == detected {
        return true;
    }

    if is_text_like(guessed) {
        // `infer` may not detect text reliably.
        return is_text_like(detected) || detected == "application/octet-stream";
    }

    // Office Open XML formats are zip containers.
    if is_ooxml(guessed) && detected == "application/zip" {
        return true;
    }

    // Legacy Office formats are often detected as octet-stream.
    if is_legacy_office(guessed) && detected == "application/octet-stream" {
        return true;
    }

    false
}

fn is_text_like(mime: &str) -> bool {
    mime.starts_with("text/") || mime == "application/json"
}

fn is_ooxml(mime: &str) -> bool {
    mime == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        || mime == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
}

fn is_legacy_office(mime: &str) -> bool {
    mime == "application/msword" || mime == "application/vnd.ms-excel"
}
