//! Attach sender machine context (IP, MAC, login user, OS) to outbound event metadata for display on peers.

use std::{
    net::UdpSocket,
    sync::OnceLock,
};

use serde_json::{json, Value};

struct SenderEnvelope {
    ip: String,
    mac: String,
    user: String,
    os: &'static str,
}

static SNAPSHOT: OnceLock<SenderEnvelope> = OnceLock::new();

/// Merge `senderIp`, `senderMac`, `senderUser`, `senderOs` into event `metadata` (camelCase JSON keys).
pub(crate) fn attach_sender_envelope(mut metadata: Value) -> Value {
    let snap = SNAPSHOT.get_or_init(compute_envelope);
    let map = match metadata.as_object_mut() {
        Some(m) => m,
        None => {
            metadata = json!({});
            metadata.as_object_mut().expect("object")
        }
    };
    map.insert("senderIp".to_string(), json!(snap.ip));
    map.insert("senderMac".to_string(), json!(snap.mac));
    map.insert("senderUser".to_string(), json!(snap.user));
    map.insert("senderOs".to_string(), json!(snap.os));
    metadata
}

fn compute_envelope() -> SenderEnvelope {
    SenderEnvelope {
        ip: primary_local_ip(),
        mac: primary_mac(),
        user: login_user(),
        os: os_label(),
    }
}

fn primary_local_ip() -> String {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return "unknown".to_string(),
    };
    if socket.connect("8.8.8.8:80").is_err() {
        return "unknown".to_string();
    }
    socket
        .local_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn primary_mac() -> String {
    #[cfg(windows)]
    {
        mac_windows_getmac()
    }
    #[cfg(not(windows))]
    {
        mac_linux_sys_class_net()
    }
}

#[cfg(windows)]
fn mac_windows_getmac() -> String {
    use std::process::Command;
    let output = match Command::new("cmd")
        .args(["/C", "getmac /fo csv /nh"])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return "unknown".to_string(),
    };
    let text = String::from_utf8_lossy(&output);
    let line = match text.lines().find(|l| !l.trim().is_empty()) {
        Some(l) => l,
        None => return "unknown".to_string(),
    };
    // First CSV field: "XX-XX-XX-XX-XX-XX" or XX-XX-...
    let raw = line
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .replace('-', ":");
    if raw.len() >= 11 && raw.matches(':').count() == 5 {
        raw.to_uppercase()
    } else {
        "unknown".to_string()
    }
}

#[cfg(not(windows))]
fn mac_linux_sys_class_net() -> String {
    use std::fs;
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return "unknown".to_string();
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        let path = format!("/sys/class/net/{name}/address");
        if let Ok(s) = fs::read_to_string(&path) {
            let mac = s.trim();
            if mac.len() >= 11 && mac.contains(':') {
                return mac.to_uppercase();
            }
        }
    }
    "unknown".to_string()
}

fn login_user() -> String {
    #[cfg(windows)]
    {
        std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "unknown".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string())
    }
}

fn os_label() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}
