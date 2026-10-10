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

fn fill_h2_v6(cfg: &Path) {
    let Ok(raw) = std::fs::read_to_string(cfg) else {
        return;
    };
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };

    let has = v
        .get("endpoint_h2_v6")
        .and_then(|x| x.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    if has {
        return;
    }

    if let Some(v6) = v.get("endpoint_v6").and_then(|x| x.as_str()).map(String::from) {
        if !v6.is_empty() && v6.contains(':') {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("endpoint_h2_v6".into(), serde_json::Value::String(v6));
                let out = serde_json::to_string_pretty(&v).unwrap_or_default();
                let _ = std::fs::write(cfg, out);
                lg!("[vpn] endpoint_h2_v6 заполнен из endpoint_v6");
            }
        }
    }
}

pub struct DnsBackup {
    iface: String,
    servers: Vec<String>,
}

fn dns_snapshot() -> Vec<DnsBackup> {
    let out = cmd("powershell")
        .args(["-NoProfile", "-Command", &format!("$a = Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object {{ $_.InterfaceAlias -ne '{}' -and $_.ServerAddresses.Count -gt 0 }}; foreach ($x in $a) {{ \"$($x.InterfaceAlias)|$($x.ServerAddresses -join ',')\" }}", tunnel_iface().unwrap_or_else(|| "usque".into()))])
        .output();

    let Ok(o) = out else { return Vec::new() };

    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|line| {
            let (iface, servers) = line.trim().split_once('|')?;
            let list: Vec<String> = servers
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if list.is_empty() {
                None
            } else {
                Some(DnsBackup { iface: iface.to_string(), servers: list })
            }
        })
        .collect()
}

fn sweep_routes() {
    let ps = format!("Get-NetRoute -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object {{ {} }} | Remove-NetRoute -Confirm:$false -ErrorAction SilentlyContinue; Get-NetRoute -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object {{ $_.DestinationPrefix -eq '162.159.192.0/20' }} | Remove-NetRoute -Confirm:$false -ErrorAction SilentlyContinue", tunnel_filter());

    if let Ok(out) = cmd("powershell").args(["-NoProfile", "-Command", &ps]).output() {
        if out.status.success() {
            lg!("[vpn] маршруты прошлой сессии очищены");
        }
    }
}

fn tunnel_iface() -> Option<String> {
    let ps = "Get-NetAdapter -Name 'usque' -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }";

    let out = cmd("powershell").args(["-NoProfile", "-Command", ps]).output().ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn tunnel_filter() -> String {
    let n = tunnel_iface().unwrap_or_else(|| "usque".into());
    format!("$_.InterfaceAlias -eq '{n}'")
}

fn has_dns(iface: &str) -> bool {
    let alias = iface.replace('\'', "''");
    let ps = format!("$a = Get-DnsClientServerAddress -InterfaceAlias '{alias}' -ErrorAction SilentlyContinue; if ($a -and $a.ServerAddresses.Count -gt 0) {{ 'yes' }}");

    cmd("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

fn clear_dns() -> Vec<DnsBackup> {
    let saved = dns_snapshot();

    if saved.is_empty() {
        return saved;
    }

    for d in &saved {
        let alias = d.iface.replace('\'', "''");

        let ps = format!("Set-DnsClientServerAddress -InterfaceAlias '{alias}' -ResetServerAddresses");

        if let Ok(out) = cmd("powershell").args(["-NoProfile", "-Command", &ps]).output() {
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                lg!("[vpn] ResetServerAddresses на {} не сработал: {err}", d.iface);
            }
        }

        if has_dns(&d.iface) {
            let _ = cmd("netsh")
                .args(["interface", "ip", "set", "dns", &format!("name={}", d.iface), "static", "0.0.0.0"])
                .output();
        }

        if has_dns(&d.iface) {
            lg!("[vpn] DNS на {} снять не удалось, оставлен как есть", d.iface);
        } else {
            lg!("[vpn] DNS на {} снят, запросы только через туннель", d.iface);
        }
    }

    let _ = cmd("ipconfig").arg("/flushdns").output();

    saved
}

fn restore_dns(saved: &[DnsBackup]) {
    if saved.is_empty() {
        return;
    }

    for d in saved {
        let list: Vec<String> = d.servers.iter().map(|s| format!("'{s}'")).collect();
        let ps = format!(
            "Set-DnsClientServerAddress -InterfaceAlias '{}' -ServerAddresses ({})",
            d.iface.replace('\'', "''"),
            list.join(",")
        );

        if let Ok(out) = cmd("powershell").args(["-NoProfile", "-Command", &ps]).output() {
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                lg!("[vpn] восстановление DNS на {} не удалось: {err}", d.iface);
            }
        }
    }

    let _ = cmd("ipconfig").arg("/flushdns").output();
    lg!("[vpn] DNS физических адаптеров восстановлен");
}

fn has_ipv6_route() -> bool {
    let out = cmd("powershell")
        .args(["-NoProfile", "-Command", "$r = Get-NetRoute -AddressFamily IPv6 -DestinationPrefix '::/0' -ErrorAction SilentlyContinue | Where-Object { $_.NextHop -ne '::' }; if ($r) { 'yes' }"])
        .output();

    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("yes"),
        Err(_) => false,
    }
}

fn cfg_endpoint(cfg: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(cfg).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let host = if v.get("endpoint_h2_v4").and_then(|x| x.as_str()).unwrap_or("").is_empty() {
        v.get("endpoint_v4")?.as_str()?.to_string()
    } else {
        v.get("endpoint_h2_v4")?.as_str()?.to_string()
    };
    Some(format!("{host}:443"))
}

fn cfg_path() -> Result<PathBuf, String> {
    let d = PathBuf::from(std::env::var("APPDATA").map_err(|e| e.to_string())?)
        .join("WARP Generator");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d.join("usque.json"))
}

fn api_reachable() -> Option<String> {
    let out = cmd("powershell")
        .args(["-NoProfile", "-Command", "$t = Test-NetConnection -ComputerName api.cloudflareclient.com -Port 443 -WarningAction SilentlyContinue -InformationLevel Quiet; if ($t) { 'ok' }"])
        .output()
        .ok()?;

    let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();

    if s.contains("ok") {
        None
    } else {
        Some("Проверка связи с api.cloudflareclient.com не пройдена: порт 443 недоступен.".into())
    }
}

fn translate_usque_error(raw: &str) -> String {
    let lower = raw.to_lowercase();

    if lower.contains("tls handshake timeout")
        || lower.contains("failed to register")
        || lower.contains("connection refused")
        || lower.contains("no such host")
    {
        return "Cloudflare не отвечает. Скорее всего, api.cloudflareclient.com заблокирован твоим провайдером — это известная проблема на части сетей. Регистрация нового аккаунта не проходит. Попробуй другой DNS или другую сеть.".into();
    }

    if lower.contains("timeout") || lower.contains("deadline exceeded") {
        return "Превышено время ожидания ответа от Cloudflare. Проверь подключение к интернету.".into();
    }

    if lower.contains("access denied") || lower.contains("permission") {
        return "Нет прав на изменение сети. Запусти приложение от имени администратора.".into();
    }

    let first = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.contains("Config file not found"))
        .unwrap_or("неизвестная ошибка");

    format!("Регистрация не удалась: {first}")
}

async fn usque_register(app: &tauri::AppHandle, cfg: &Path) -> Result<(), String> {
    let mut last = String::new();

    for attempt in 1..=3u32 {
        if attempt > 1 {
            lg!("[vpn] попытка регистрации {attempt} из 3");
            tokio::time::sleep(std::time::Duration::from_secs(3u64 * attempt as u64)).await;
        }

        match register_once(app, cfg).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e;
                let soft = last.to_lowercase().contains("tls handshake timeout")
                    || last.to_lowercase().contains("таймаут")
                    || last.to_lowercase().contains("превышено время");

                if !soft {
                    break;
                }
            }
        }
    }

    Err(last)
}

async fn register_once(app: &tauri::AppHandle, cfg: &Path) -> Result<(), String> {
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
                    lg!("[vpn] регистрация провалилась, код {code}: {}", err.trim());
                    Err(translate_usque_error(&err))
                };
            }
            _ => {}
        }
    }
    Err("регистрация прервана".into())
}

#[derive(Clone, serde::Serialize)]
pub struct Applied {
    pub dns: String,
    pub mtu: u32,
    pub transport: String,
    pub keepalive: String,
    pub ipv6: bool,
    pub profile: String,
}

#[derive(Clone, serde::Serialize)]
pub struct VpnStatus {
    pub running: bool,
    pub iface: Option<String>,
    pub endpoint: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub connected_since: Option<u64>,
    pub applied: Option<Applied>,
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
            applied: None,
        }
    }
}

pub struct VpnManager {
    status: Arc<Mutex<VpnStatus>>,
    child: Arc<Mutex<Option<CommandChild>>>,
    dns_backup: Arc<Mutex<Vec<DnsBackup>>>,
}

impl VpnManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(VpnStatus::default())),
            child: Arc::new(Mutex::new(None)),
            dns_backup: Arc::new(Mutex::new(Vec::new())),
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
        restore_dns(&dns_snapshot());
        sweep_routes();

        let include_ipv6 = options.ipv6 == "enabled";
        let use_ipv6 = include_ipv6 && has_ipv6_route();
        if include_ipv6 && !use_ipv6 {
            lg!("[vpn] IPv6 включён, но маршрута ::/0 нет — транспорт через IPv4");
        }
        let dns = crate::build_dns_line(&options.dns, include_ipv6);
        let mtu = if options.mtu == 0 { 1280 } else { options.mtu };
        let profile = options.profile.as_deref().unwrap_or("standard").to_string();
        let keepalive = if profile == "paranoid" {
            60
        } else {
            options.keepalive
        };
        let transport = if profile == "standard" { "HTTP/3 (QUIC)" } else { "HTTP/2 (TCP)" };
        let endpoint_label;

        if let Err(e) = setup_exclusion_route() {
            lg!("[vpn] exclusion route warning: {e}");
        }

        let binaries_dir = sidecar_dir(app)?;
        lg!("[vpn] sidecar dir: {}", binaries_dir.display());

        let cfg = cfg_path()?;
        if !cfg.exists() {
            if let Some(problem) = api_reachable() {
                lg!("[vpn] предварительная проверка: {problem}");
            }
            usque_register(app, &cfg).await?;
        }

        endpoint_label = cfg_endpoint(&cfg).unwrap_or_else(|| "162.159.198.2:443".into());

        if use_ipv6 && profile != "standard" {
            fill_h2_v6(&cfg);
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
        if use_ipv6 {
            args.push("-6".into());
        }
        if keepalive > 0 {
            args.push("-k".into());
            args.push(format!("{keepalive}s"));
        }

        lg!("[vpn] {} | mtu {mtu} | keepalive {keepalive}s | ipv6 {use_ipv6}", transport);

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
        let dns_backup = Arc::clone(&self.dns_backup);

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

                    let saved = clear_dns();
                    lg!("[vpn] DNS снят с {} адаптеров, запросы идут только через туннель", saved.len());
                    *dns_backup.lock().unwrap() = saved;

                    let mut s = status.lock().unwrap();
                    s.running = true;
                    s.iface = tunnel_iface().or_else(|| Some("usque".into()));
                    s.endpoint = Some(endpoint_label.clone());
                    s.applied = Some(Applied {
                        dns: dns_ref.to_string(),
                        mtu,
                        transport: transport.to_string(),
                        keepalive: if keepalive > 0 {
                            format!("{keepalive}s")
                        } else {
                            "по умолчанию (30s)".into()
                        },
                        ipv6: use_ipv6,
                        profile: profile.to_string(),
                    });
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
                        let saved: Vec<DnsBackup> = dns_backup.lock().unwrap().drain(..).collect();
                        restore_dns(&saved);
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
            let mut guard = self.dns_backup.lock().unwrap();
            let saved = std::mem::take(&mut *guard);
            restore_dns(&saved);
        }

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
        .args(["-NoProfile", "-Command", &format!("$s = Get-NetAdapterStatistics -Name '{}' -ErrorAction SilentlyContinue; if ($s) {{ \"$($s.ReceivedBytes)|$($s.SentBytes)\" }}", tunnel_iface().unwrap_or_else(|| "usque".into()))])
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
        .args(["-NoProfile", "-Command", &format!("$r = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Where-Object {{ $_.NextHop -ne '0.0.0.0' -and $_.InterfaceAlias -ne '{}' }} | Sort-Object RouteMetric | Select-Object -First 1; if ($r) {{ \"$($r.NextHop)|$($r.InterfaceIndex)\" }}", tunnel_iface().unwrap_or_else(|| "usque".into()))])
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
    let name = tunnel_iface().ok_or_else(|| "туннельный адаптер не найден".to_string())?;
    let idx = get_iface_idx(&name).ok_or_else(|| format!("индекс интерфейса {name} не найден"))?;

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
    if let Some(idx) = tunnel_iface().and_then(|n| get_iface_idx(&n)) {
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

    let ps = format!("Set-DnsClientServerAddress -InterfaceAlias '{}' -ServerAddresses ({list})", tunnel_iface().unwrap_or_else(|| "usque".into()));

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
pub fn vpn_emergency_reset() -> Result<String, String> {
    let mut done: Vec<String> = Vec::new();

    for p in &["warp-generator-desktop", "usque"] {
        cmd("taskkill")
            .args(["/F", "/IM", &format!("{p}.exe"), "/T"])
            .output()
            .ok();
    }
    done.push("процессы приложения и туннеля завершены".into());

    let _ = cmd("powershell")
        .args(["-NoProfile", "-Command", "Remove-NetAdapter -Name 'usque' -Confirm:$false -ErrorAction SilentlyContinue"])
        .output();
    done.push("адаптер usque удалён".into());

    let ps = "Get-NetRoute -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -eq '0.0.0.0/0' -and $_.InterfaceAlias -eq 'usque' } | Remove-NetRoute -Confirm:$false -ErrorAction SilentlyContinue; Get-NetRoute -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -eq '162.159.192.0/20' } | Remove-NetRoute -Confirm:$false -ErrorAction SilentlyContinue";
    let _ = cmd("powershell").args(["-NoProfile", "-Command", ps]).output();
    done.push("маршруты туннеля сняты".into());

    let _ = cmd("powershell")
        .args(["-NoProfile", "-Command", &format!("Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object {{ {} }} | Reset-DnsClientServerAddress -ErrorAction SilentlyContinue", tunnel_filter())])
        .output();
    let _ = cmd("ipconfig").arg("/flushdns").output();
    done.push("сброшены DNS и кэш резолвера".into());

    let left = cmd("powershell")
        .args(["-NoProfile", "-Command", &format!("$r = Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Where-Object {{ {} }}; if ($r) {{ 'ok' }}", format!("$_.InterfaceAlias -ne '{}'", tunnel_iface().unwrap_or_else(|| "usque".into())))])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("ok"))
        .unwrap_or(false);

    if left {
        lg!("[vpn] аварийный сброс выполнен, маршрут по умолчанию на месте");
        Ok(done.join("; "))
    } else {
        lg!("[vpn] аварийный сброс: маршрут по умолчанию не найден");
        Err("Маршрут по умолчанию не восстановился. Перезагрузи сетевой адаптер в настройках Windows или выполни ipconfig /flushdns.".into())
    }
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