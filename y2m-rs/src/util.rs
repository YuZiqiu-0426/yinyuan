use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use uuid::Uuid;
use y2m_common::EventPacket;

pub(crate) fn resolve_config_path(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(y2m_client_core::ClientConfig::default_config_path)
}

pub(crate) fn load_or_default_config(path: &Path) -> anyhow::Result<y2m_client_core::ClientConfig> {
    if path.exists() {
        y2m_client_core::ClientConfig::load_from_path(path)
    } else {
        Ok(y2m_client_core::ClientConfig::default())
    }
}

pub(crate) fn parse_json_value(content: &str) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::from_str(content)?)
}

pub(crate) fn parse_file_id(packet: &EventPacket) -> anyhow::Result<Uuid> {
    let file_id = packet.payload.metadata.get("fileId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing fileId"))?;
    Ok(Uuid::parse_str(file_id)?)
}

pub(crate) fn matches_file_id(packet: &EventPacket, file_id: Uuid) -> bool {
    parse_file_id(packet).map(|v| v == file_id).unwrap_or(false)
}

pub(crate) fn ensure_unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() { return path; }
    let stem = path.file_stem().map(|v| v.to_string_lossy().to_string()).unwrap_or_else(|| "file".to_string());
    let ext = path.extension().map(|v| v.to_string_lossy().to_string());
    let parent = path.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    for index in 1.. {
        let name = match &ext {
            Some(ext) => format!("{stem}-{index}.{ext}"),
            None => format!("{stem}-{index}"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() { return candidate; }
    }
    path
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().as_slice().iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB { format!("{:.2} GiB", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.2} MiB", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.2} KiB", bytes as f64 / KB as f64) }
    else { format!("{bytes} B") }
}

pub(crate) fn guess_content_type(path: &Path) -> String {
    match path.extension().and_then(|v| v.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "txt" | "md" | "log" => "text/plain".to_string(),
        "json" => "application/json".to_string(),
        "html" | "htm" => "text/html".to_string(),
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "pdf" => "application/pdf".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

/// Strip ANSI OSC/CSI and non-printable controls (keep `\n` / `\t`) for safe console output.
pub(crate) fn sanitize_terminal_controls(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut iter = input.chars().peekable();
    while let Some(ch) = iter.next() {
        if ch == '\u{1b}' {
            match iter.peek().copied() {
                Some('[') => {
                    iter.next();
                    while let Some(c) = iter.next() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    iter.next();
                    while let Some(c) = iter.next() {
                        if c == '\u{07}' {
                            break;
                        }
                        if c == '\u{1b}' && matches!(iter.peek(), Some('\\')) {
                            iter.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    iter.next();
                }
                None => break,
            }
            continue;
        }
        out.push(ch);
    }
    out.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}
