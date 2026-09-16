mod i1_masks;
mod quic;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use x25519_dalek::{PublicKey, StaticSecret};

const CF_BASE: &str = "https://api.cloudflareclient.com/v0i1909051800";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateOptions {
    pub format: String,
    pub connection: String,
    pub dns: String,
    pub endpoint: String,
    pub exclude_lan: bool,
    pub ipv6: String,
    pub keepalive: u32,
    pub mtu: u32,
    pub custom_i1_domain: Option<String>,
}

#[derive(Debug, Serialize)]
struct CfRegRequest {
    install_id: String,
    tos: String,
    key: String,
    fcm_token: String,
    #[serde(rename = "type")]
    kind: String,
    locale: String,
}

#[derive(Debug, Deserialize)]
struct CfRegResponse {
    result: CfRegResult,
}

#[derive(Debug, Deserialize)]
struct CfRegResult {
    id: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct CfWarpResponse {
    result: CfWarpResult,
}

#[derive(Debug, Deserialize)]
struct CfWarpResult {
    config: CfConfig,
}

#[derive(Debug, Deserialize)]
struct CfConfig {
    peers: Vec<CfPeer>,
    interface: CfInterface,
}

#[derive(Debug, Deserialize)]
struct CfPeer {
    public_key: String,
}

#[derive(Debug, Deserialize)]
struct CfInterface {
    addresses: CfAddresses,
}

#[derive(Debug, Deserialize)]
struct CfAddresses {
    v4: String,
    v6: String,
}

fn generate_keypair() -> Result<(String, String), String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| e.to_string())?;

    // WireGuard clamping
    bytes[0] &= 248;
    bytes[31] &= 127;
    bytes[31] |= 64;

    let secret = StaticSecret::from(bytes);
    let public = PublicKey::from(&secret);

    Ok((
        BASE64.encode(secret.to_bytes()),
        BASE64.encode(public.as_bytes()),
    ))
}

async fn register_client(
    client: &reqwest::Client,
    public_key: &str,
) -> Result<(String, String), String> {
    let body = CfRegRequest {
        install_id: String::new(),
        tos: "2020-08-18T00:00:00.000Z".into(),
        key: public_key.to_string(),
        fcm_token: String::new(),
        kind: "ios".into(),
        locale: "en_US".into(),
    };

    let resp = client
        .post(format!("{CF_BASE}/reg"))
        .header("User-Agent", "okhttp/3.12.1")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("registration failed: HTTP {}", resp.status()));
    }

    let data: CfRegResponse = resp.json().await.map_err(|e| format!("parse: {e}"))?;

    if data.result.id.is_empty() || data.result.token.is_empty() {
        return Err("invalid registration response".into());
    }

    Ok((data.result.id, data.result.token))
}

async fn enable_warp(
    client: &reqwest::Client,
    client_id: &str,
    token: &str,
) -> Result<CfWarpResponse, String> {
    let resp = client
        .patch(format!("{CF_BASE}/reg/{client_id}"))
        .header("User-Agent", "okhttp/3.12.1")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(r#"{"warp_enabled":true}"#)
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("enable WARP failed: HTTP {}", resp.status()));
    }

    let data: CfWarpResponse = resp.json().await.map_err(|e| format!("parse: {e}"))?;

    if data.result.config.peers.is_empty() {
        return Err("no peers in WARP config".into());
    }

    Ok(data)
}

fn build_dns_line(provider: &str, include_ipv6: bool) -> String {
    match provider {
        "8.8.8.8" => {
            if include_ipv6 {
                "8.8.8.8, 8.8.4.4, 2001:4860:4860::8888, 2001:4860:4860::8844".into()
            } else {
                "8.8.8.8, 8.8.4.4".into()
            }
        }
        "9.9.9.9" => {
            if include_ipv6 {
                "9.9.9.9, 149.112.112.112, 2620:fe::fe, 2620:fe::9".into()
            } else {
                "9.9.9.9, 149.112.112.112".into()
            }
        }
        "dns.malw.link" => {
            if include_ipv6 {
                "84.21.189.133, 193.23.209.189, 2a12:bec4:1460:294::2, 2a01:ecc0:680:120::2".into()
            } else {
                "84.21.189.133, 193.23.209.189".into()
            }
        }
        "xbox-dns.ru" => {
            if include_ipv6 {
                "111.88.96.50, 111.88.96.51, 2a00:ab00:1233:26::50, 2a00:ab00:1233:26::51".into()
            } else {
                "111.88.96.50, 111.88.96.51".into()
            }
        }
        "dns.geohide.ru" => {
            "45.155.204.190, 37.230.192.51".into()
        }
        "dns.comss.one" => {
            if include_ipv6 {
                "83.220.169.155, 212.109.195.93, 195.133.25.16, 2a01:230:4:915::2, 2a01:230:4:306::2".into()
            } else {
                "83.220.169.155, 212.109.195.93, 195.133.25.16".into()
            }
        }
        "dns.mafioznik.xyz" => {
            "103.27.157.38, 103.27.157.100".into()
        }
        _ => {
            if include_ipv6 {
                "1.1.1.1, 1.0.0.1, 2606:4700:4700::1111, 2606:4700:4700::1001".into()
            } else {
                "1.1.1.1, 1.0.0.1".into()
            }
        }
    }
}

fn build_allowed_ips(exclude_lan: bool, include_ipv6: bool) -> String {
    if exclude_lan {
        let base = "1.0.0.0/8, 2.0.0.0/7, 4.0.0.0/6, 8.0.0.0/7, 11.0.0.0/8, 12.0.0.0/6, 16.0.0.0/4, 32.0.0.0/3, 64.0.0.0/3, 96.0.0.0/4, 112.0.0.0/5, 120.0.0.0/6, 124.0.0.0/7, 126.0.0.0/8, 128.0.0.0/3, 160.0.0.0/5, 168.0.0.0/8, 169.0.0.0/9, 169.128.0.0/10, 169.192.0.0/11, 169.224.0.0/12, 169.240.0.0/13, 169.248.0.0/14, 169.252.0.0/15, 169.255.0.0/16, 170.0.0.0/7, 172.0.0.0/12, 172.32.0.0/11, 172.64.0.0/10, 172.128.0.0/9, 173.0.0.0/8, 174.0.0.0/7, 176.0.0.0/4, 192.0.0.0/9, 192.128.0.0/11, 192.160.0.0/13, 192.169.0.0/16, 192.170.0.0/15, 192.172.0.0/14, 192.176.0.0/12, 192.192.0.0/10, 193.0.0.0/8, 194.0.0.0/7, 196.0.0.0/6, 200.0.0.0/5, 208.0.0.0/4, 224.0.0.0/4";
        if include_ipv6 {
            return format!("{base}, ::/1, 8000::/2, c000::/3, e000::/4, f000::/5, f800::/6, fe00::/9, fec0::/10, ff00::/8");
        }
        return base.into();
    }

    if include_ipv6 {
        "0.0.0.0/0, ::/0".into()
    } else {
        "0.0.0.0/0".into()
    }
}

async fn build_config(options: GenerateOptions) -> Result<String, String> {
    let include_ipv6 = options.ipv6 == "enabled";
    let is_amnezia = options.connection.starts_with("amneziawg");

    let (private_key, public_key) = generate_keypair()?;

    let client = reqwest::Client::new();
    let (client_id, token) = register_client(&client, &public_key).await?;
    let warp = enable_warp(&client, &client_id, &token).await?;

    let peer = warp.result.config.peers.first().ok_or("no peers")?;
    let iface = &warp.result.config.interface;

    let address = if include_ipv6 {
        format!("{}, {}", iface.addresses.v4, iface.addresses.v6)
    } else {
        iface.addresses.v4.clone()
    };

    let dns = build_dns_line(&options.dns, include_ipv6);
    let allowed = build_allowed_ips(options.exclude_lan, include_ipv6);

    let mut out = String::new();
    out.push_str("[Interface]\n");
    out.push_str(&format!("PrivateKey = {private_key}\n"));
    out.push_str(&format!("Address = {address}\n"));
    out.push_str(&format!("DNS = {dns}\n"));
    out.push_str(&format!("MTU = {}\n", options.mtu));

    if is_amnezia {
        out.push_str("S1 = 0\n");
        out.push_str("S2 = 0\n");
        out.push_str("Jc = 4\n");
        out.push_str("Jmin = 40\n");
        out.push_str("Jmax = 70\n");
        out.push_str("H1 = 1\n");
        out.push_str("H2 = 2\n");
        out.push_str("H3 = 3\n");
        out.push_str("H4 = 4\n");

        if options.connection == "amneziawg15" {
            let i1_line = match options.custom_i1_domain.as_deref() {
                Some(d) if !d.trim().is_empty() => quic::generate_i1(d)?,
                _ => i1_masks::pick().to_string(),
            };
            out.push_str(&i1_line);
            out.push('\n');
        }
    }

    out.push_str("\n[Peer]\n");
    out.push_str(&format!("PublicKey = {}\n", peer.public_key));
    out.push_str(&format!("AllowedIPs = {allowed}\n"));
    out.push_str(&format!("Endpoint = {}\n", options.endpoint));

    if options.keepalive > 0 {
        out.push_str(&format!("PersistentKeepalive = {}\n", options.keepalive));
    }

    Ok(out)
}

fn user_home() -> Result<PathBuf, String> {
    let var = if cfg!(target_os = "windows") {
        "USERPROFILE"
    } else {
        "HOME"
    };
    let home = std::env::var(var).map_err(|e| e.to_string())?;
    Ok(PathBuf::from(home))
}

fn configs_dir() -> Result<PathBuf, String> {
    let dir = user_home()?.join("Downloads").join("WARP Generator");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn find_amnezia_exe() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from(r"C:\Program Files\AmneziaWG\amneziawg.exe"),
        PathBuf::from(r"C:\Program Files (x86)\AmneziaWG\amneziawg.exe"),
    ];

    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(&local)
                .join("Programs")
                .join("AmneziaWG")
                .join("amneziawg.exe"),
        );
    }

    candidates.into_iter().find(|p| p.exists())
}

#[tauri::command]
async fn generate_warp_config(options: GenerateOptions) -> Result<String, String> {
    build_config(options).await
}

#[tauri::command]
async fn save_to_path(path: String, content: String) -> Result<String, String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

#[tauri::command]
async fn open_in_amnezia(content: String, filename: String) -> Result<String, String> {
    let dir = configs_dir()?;
    let path = dir.join(&filename);
    std::fs::write(&path, content).map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(exe) = find_amnezia_exe() {
            std::process::Command::new(exe)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;

        let _ = std::process::Command::new("open")
            .arg("-a")
            .arg("AmneziaWG")
            .spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(dir.to_string_lossy().to_string())
            .spawn();
    }

    Ok(path.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            generate_warp_config,
            save_to_path,
            open_in_amnezia
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}