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
    #[cfg(target_os = "macos")]
    {
        mac_macos_ifconfig()
    }
    #[cfg(target_os = "linux")]
    {
        mac_linux_sys_class_net()
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        "unknown".to_string()
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

#[cfg(target_os = "macos")]
fn mac_macos_ifconfig() -> String {
    use std::process::Command;
    let output = match Command::new("/sbin/ifconfig").output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => return "unknown".to_string(),
    };
    let text = String::from_utf8_lossy(&output);
    parse_macos_ifconfig_ether(&text).unwrap_or_else(|| "unknown".to_string())
}

/// Parse `ifconfig` output: each interface block has `name: flags=...` then indented `ether aa:bb:...`.
#[cfg(target_os = "macos")]
fn parse_macos_ifconfig_ether(text: &str) -> Option<String> {
    let mut current_iface = String::new();
    for line in text.lines() {
        let line = line.trim_end();
        if !line.starts_with('\t') && line.contains(':') && line.contains("flags=") {
            if let Some(colon) = line.find(':') {
                current_iface = line[..colon].trim().to_string();
            }
            continue;
        }
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("ether ") else { continue };
        if current_iface == "lo0" || current_iface.is_empty() {
            continue;
        }
        let token = rest.split_whitespace().next()?;
        if !mac_string_is_valid_colon_hex(token) {
            continue;
        }
        return Some(token.to_uppercase());
    }
    None
}

#[cfg(target_os = "macos")]
fn mac_string_is_valid_colon_hex(s: &str) -> bool {
    if s.matches(':').count() != 5 {
        return false;
    }
    if s.eq_ignore_ascii_case("00:00:00:00:00:00") {
        return false;
    }
    s.split(':').all(|p| p.len() == 2 && u8::from_str_radix(p, 16).is_ok())
}

#[cfg(target_os = "linux")]
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

#[cfg(all(test, target_os = "macos"))]
mod macos_parse_tests {
    use super::parse_macos_ifconfig_ether;

    #[test]
    fn skips_lo0_and_returns_first_en_ether() {
        let sample = "\
lo0: flags=8049<UP,LOOPBACK,RUNNING,MULTICAST> mtu 16384
\tinet 127.0.0.1 netmask 0xff000000
en0: flags=8863<UP,BROADCAST,RUNNING> mtu 1500
\tether aa:bb:cc:dd:ee:ff
en1: flags=8863<UP,BROADCAST,RUNNING> mtu 1500
\tether 11:22:33:44:55:66
";
        assert_eq!(
            parse_macos_ifconfig_ether(sample).as_deref(),
            Some("AA:BB:CC:DD:EE:FF")
        );
    }
}
