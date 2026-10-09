use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

pub fn lg(m: String) {
    eprintln!("{m}");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(
            std::env::var_os("TEMP")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("warp-gen.log"),
        )
    {
        let _ = writeln!(f, "{m}");
    }
}

macro_rules! lg {
    ($($a:tt)*) => { $crate::vpn::lg(format!($($a)*)) };
}

fn sidecar_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let mut c = vec![app.path().resource_dir().map_err(|e| e.to_string())?];
    if let Ok(e) = std::env::current_exe() {
        if let Some(d) = e.parent() {
            c.push(d.to_path_buf());
        }
    }
    if let Ok(d) = std::env::current_dir() {
        c.push(d.join("binaries"));
    }
    for d in c {
        if d.join("wintun.dll").exists() {
            return Ok(d);
        }
    }
    Err("wintun.dll не найден".into())
}

fn cfg_path() -> Result<PathBuf, String> {
    let d = PathBuf::from(std::env::var("APPDATA").map_err(|e| e.to_string())?)
        .join("WARP Generator");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d.join("usque.json"))
}

async fn usque_register(app: &tauri::AppHandle, cfg: &Path) -> Result<(), String> {
    lg!("[vpn] регистрация нового аккаунта usque");
    let (mut rx, _ch) = app
        .shell()
        .sidecar("usque")
        .map_err(|e| e.to_string())?
        .args(["register", "-a", "-c", &cfg.to_string_lossy()])
        .spawn()
        .map_err(|e| e.to_string())?;

    let mut err = String::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CommandEvent::Stdout(b) => lg!("[usque] {}", String::from_utf8_lossy(&b).trim()),
            CommandEvent::Stderr(b) => err.push_str(&String::from_utf8_lossy(&b)),
            CommandEvent::Terminated(p) => {
                let code = p.code.unwrap_or(-1);
                return if code == 0 {
                    lg!("[vpn] регистрация успешна");
                    Ok(())
                } else {
                    Err(if err.trim().is_empty() {
                        format!("регистрация провалилась (код {code})")
                    } else {
                        err.trim().to_string()
                    })
                };
            }
            _ => {}
        }
    }
    Err("регистрация прервана".into())
}

#[derive(Clone, serde::Serialize)]
pub struct VpnStatus {
    pub running: bool,
    pub iface: Option<String>,
    pub endpoint: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub connected_since: Option<u64>,
}

impl Default for VpnStatus {
    fn default() -> Self {
        Self {
            running: false,
            iface: None,
            endpoint: None,
            rx_bytes: 0,
            tx_bytes: 0,
            connected_since: None,
        }
    }
}

pub struct VpnManager {
    status: Arc<Mutex<VpnStatus>>,
    child: Arc<Mutex<Option<CommandChild>>>,
}

impl VpnManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(VpnStatus::default())),
            child: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect(
        &self,
        app: &tauri::AppHandle,
        options: crate::GenerateOptions,
    ) -> Result<(), String> {
        {
            let s = self.status.lock().unwrap();
            if s.running {
                return Err("VPN уже запущен".into());
            }
        }

        if !is_admin() {
            return Err(
                "Требуются права администратора. Закройте приложение и запустите от имени администратора."
                    .into(),
            );
        }

        cleanup_stale_adapter();

        let include_ipv6 = options.ipv6 == "enabled";
        let dns = crate::build_dns_line(&options.dns, include_ipv6);
        let mtu = if options.mtu == 0 { 1280 } else { options.mtu };
        let profile = options.profile.as_deref().unwrap_or("standard");

        if let Err(e) = setup_exclusion_route() {
            lg!("[vpn] exclusion route warning: {e}");
        }

        let binaries_dir = sidecar_dir(app)?;
        lg!("[vpn] sidecar dir: {}", binaries_dir.display());

        let cfg = cfg_path()?;
        if !cfg.exists() {
            usque_register(app, &cfg).await?;
        }

        let shell = app.shell();

        let cfg_arg = cfg.to_string_lossy().to_string();
        let mut args: Vec<String> = vec![
            "-c".into(),
            cfg_arg,
            "nativetun".into(),
            "-n".into(),
            "usque".into(),
            "-m".into(),
            mtu.to_string(),
            "--always-reconnect".into(),
        ];
        if profile != "standard" {
            args.push("--http2".into());
        }
        if profile == "paranoid" {
            args.push("-k".into());
            args.push("60s".into());
        }

        let (mut rx, child) = shell
            .sidecar("usque")
            .map_err(|e| format!("sidecar not found: {e}"))?
            .current_dir(&binaries_dir)
            .args(args)
            .spawn()
            .map_err(|e| format!("spawn failed: {e}"))?;

        {
            let mut guard = self.child.lock().unwrap();
            *guard = Some(child);
        }

        let status = Arc::clone(&self.status);

        tauri::async_runtime::spawn(async move {
            let mut ready = false;

            let handle_line = |line: &str, status: &Arc<Mutex<VpnStatus>>, ready: &mut bool| {
                if !*ready && line.contains("Connected to MASQUE server") {
                    *ready = true;
                    lg!("[vpn] detected MASQUE connected, setting up routes...");

                    std::thread::sleep(std::time::Duration::from_millis(1500));

                    if let Err(e) = setup_main_route() {
                        lg!("[vpn] setup_main_route failed: {e}");
                        let mut s = status.lock().unwrap();
                        s.running = false;
                        return;
                    }

                    let dns_ref: &str = if dns.is_empty() { "1.1.1.1" } else { &dns };
                    lg!("[vpn] DNS из генератора: {dns_ref}");
                    if let Err(e) = setup_dns(dns_ref) {
                        lg!("[vpn] setup_dns warning: {e}");
                    }

                    let mut s = status.lock().unwrap();
                    s.running = true;
                    s.iface = Some("usque".into());
                    s.endpoint = Some("162.159.198.2:443".into());
                    s.connected_since = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                    );
                }
            };

            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            lg!("[usque] {line}");
                            handle_line(line, &status, &mut ready);
                        }
                    }
                    CommandEvent::Stderr(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            lg!("[usque:err] {line}");
                            handle_line(line, &status, &mut ready);
                        }
                    }
                    CommandEvent::Terminated(_) => {
                        let _ = teardown_main_route();
                        let mut s = status.lock().unwrap();
                        s.running = false;
                        s.iface = None;
                        s.endpoint = None;
                        s.connected_since = None;
                        s.rx_bytes = 0;
                        s.tx_bytes = 0;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), String> {
        let _ = teardown_main_route();

        {
            let mut guard = self.child.lock().unwrap();
            if let Some(child) = guard.take() {
                let _ = child.kill();
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(500));

        cleanup_stale_adapter();

        let mut s = self.status.lock().unwrap();
        s.running = false;
        s.iface = None;
        s.endpoint = None;
        s.connected_since = None;
        s.rx_bytes = 0;
        s.tx_bytes = 0;

        Ok(())
    }

    pub async fn get_status(&self) -> VpnStatus {
        let mut s = self.status.lock().unwrap().clone();

        if s.running {
            let (rx, tx) = get_adapter_stats();
            s.rx_bytes = rx;
            s.tx_bytes = tx;
        }

        s
    }
}

fn get_adapter_stats() -> (u64, u64) {
    let output = cmd("powershell")
        .args(["-NoProfile", "-Command", "$s = Get-NetAdapterStatistics -Name 'usque' -ErrorAction SilentlyContinue; if ($s) { \"$($s.ReceivedBytes)|$($s.SentBytes)\" }"])
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        let text = text.trim();
        if !text.is_empty() {
            let mut p = text.split('|');
            let rx: u64 = p.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let tx: u64 = p.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            return (rx, tx);
        }
    }

    (0, 0)
}

pub fn cleanup_stale_adapter() {
    let _ = cmd("powershell")
        .args(["-NoProfile", "-Command", "Stop-Process -Name usque -Force -ErrorAction SilentlyContinue; Remove-NetAdapter -Name 'usque' -Confirm:$false -ErrorAction SilentlyContinue"])
        .output();

    std::thread::sleep(std::time::Duration::from_millis(800));
    lg!("[vpn] cleanup stale adapter done");
}

fn cmd(p: &str) -> Command {
    let mut c = Command::new(p);
    c.creation_flags(0x08000000);
    c
}

fn is_admin() -> bool {
    cmd("net")
        .arg("session")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn get_iface_idx(name: &str) -> Option<u32> {
    let output = cmd("netsh")
        .args(["interface", "ipv4", "show", "interfaces"])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.contains(name) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(idx_str) = parts.first() {
                return idx_str.parse().ok();
            }
        }
    }
    None
}

fn get_default_gateway_and_idx() -> Option<(String, u32)> {
    let output = cmd("powershell")
        .args(["-NoProfile", "-Command", "$r = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Where-Object { $_.NextHop -ne '0.0.0.0' -and $_.InterfaceAlias -ne 'usque' } | Sort-Object RouteMetric | Select-Object -First 1; if ($r) { \"$($r.NextHop)|$($r.InterfaceIndex)\" }"])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.trim();
    let mut parts = text.split('|');
    let gw = parts.next()?.trim().to_string();
    let idx: u32 = parts.next()?.trim().parse().ok()?;

    if gw.is_empty() {
        return None;
    }

    Some((gw, idx))
}

fn setup_exclusion_route() -> Result<(), String> {
    let (gw, idx) = get_default_gateway_and_idx()
        .ok_or_else(|| "default gateway not found".to_string())?;

    let _ = cmd("route")
        .args(["delete", "162.159.192.0", "mask", "255.255.240.0"])
        .output();

    let output = cmd("route")
        .args(["add", "162.159.192.0", "mask", "255.255.240.0", &gw, "metric", "1", "if", &idx.to_string()])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "exclusion route add: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    lg!("[vpn] exclusion route via {gw} if {idx}");
    Ok(())
}

fn setup_main_route() -> Result<(), String> {
    let idx = get_iface_idx("usque").ok_or_else(|| "usque interface not found".to_string())?;

    let output = cmd("route")
        .args(["add", "0.0.0.0", "mask", "0.0.0.0", "0.0.0.0", "metric", "1", "if", &idx.to_string()])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "main route add: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    lg!("[vpn] main route via usque if {idx}");
    Ok(())
}

fn teardown_main_route() -> Result<(), String> {
    if let Some(idx) = get_iface_idx("usque") {
        let _ = cmd("route")
            .args(["delete", "0.0.0.0", "mask", "0.0.0.0", "if", &idx.to_string()])
            .output();
        lg!("[vpn] main route removed if {idx}");
    }
    Ok(())
}

fn setup_dns(dns: &str) -> Result<(), String> {
    let servers: Vec<&str> = dns
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.contains(':'))
        .take(2)
        .collect();

    let servers = if servers.is_empty() {
        vec!["1.1.1.1"]
    } else {
        servers
    };

    let list = servers
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");

    let ps = format!("Set-DnsClientServerAddress -InterfaceAlias 'usque' -ServerAddresses ({list})");

    let output = cmd("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    lg!("[vpn] DNS set: {:?}", servers);
    Ok(())
}

#[tauri::command]
pub async fn vpn_connect(
    app: tauri::AppHandle,
    options: crate::GenerateOptions,
    state: tauri::State<'_, VpnManager>,
) -> Result<(), String> {
    state.connect(&app, options).await
}

#[tauri::command]
pub async fn vpn_disconnect(state: tauri::State<'_, VpnManager>) -> Result<(), String> {
    state.disconnect().await
}

#[tauri::command]
pub async fn vpn_get_status(state: tauri::State<'_, VpnManager>) -> Result<VpnStatus, String> {
    Ok(state.get_status().await)
}

#[tauri::command]
pub async fn app_hide(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn app_quit(
    app: tauri::AppHandle,
    state: tauri::State<'_, VpnManager>,
    keepvpn: Option<bool>,
) -> Result<(), String> {
    if keepvpn != Some(true) {
        let _ = state.disconnect().await;
        std::thread::sleep(std::time::Duration::from_millis(300));
    } else if let Some(c) = state.child.lock().unwrap().take() {
        let _ = c.kill();
    }
    app.exit(0);
    Ok(())
}