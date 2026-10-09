# WARP Generator Desktop

[Русский](README_ru.md) | **English**

!!! MAY NOT WORK WELL WITH MOSCOW-BASED ISPS !!!
In the next update, I will add a relay to bypass the ISP-level blocking of the Cloudflare API.

![WARP Generator Desktop](.github/assets/screenshot-generator.png)

Desktop application for generating Cloudflare WARP and AmneziaWG configurations. Built with Tauri and Rust.

The project is a desktop fork inspired by the original web version at [warp3.llimonix.pw](https://warp3.llimonix.pw), made by [nellimonix](https://github.com/nellimonix). Original author is credited at the bottom.

## About this fork

This is my personal fork with my own UI, branding and features. You can find the web version of the same project here:

- Website: https://warp.sakeen.ru
- GitHub (web): https://github.com/Thiefgg/warp-config-generator-vercel
- GitHub (desktop): https://github.com/Thiefgg/warp-generator-desktop

## Screenshots

Main interface:

![Generator](.github/assets/screenshot-generator.png)

All generator options expanded:

![Options](.github/assets/screenshot-options.png)

After clicking "Open in AmneziaWG" — the config is saved and AmneziaWG opens with import instructions:

![AmneziaWG import](.github/assets/screenshot-amnezia.png)

## Installation

Download the latest installer from [Releases](https://github.com/Thiefgg/warp-generator-desktop/releases).

| File | Description |
| --- | --- |
| `WARP Generator_x.x.x_x64-setup.exe` | NSIS installer (recommended) |
| `WARP Generator_x.x.x_x64_en-US.msi` | MSI package |

Requires Windows 10 (2020 or newer) or Windows 11. WebView2 is already included in both.

## Building from source

You will need:

- Node.js 18 or newer
- Rust 1.75 or newer
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS

```bash
git clone https://github.com/Thiefgg/warp-generator-desktop.git
cd warp-generator-desktop
npm install
npm run tauri dev      # development
npm run tauri build    # production installer
```

The installer will be in the bundle path configured in `src-tauri/.cargo/config.toml` (default `C:\Users\<user>\AppData\Local\cargo-target\warp-generator\release\bundle\`).

## Config formats

| Format | Description |
| --- | --- |
| WireGuard | Standard config, works with any WireGuard client |
| AmneziaWG | AmneziaWG obfuscation parameters |
| Sakeen | Custom format with a `[Stealth]` section |

### Sakeen

The app's own format. Unlike AmneziaWG, the masking domain is picked at random on every generation from a list of allowed resources, so the handshake pattern never repeats and cannot be learned.

Three profiles:

| Profile | Transport | Purpose |
| --- | --- | --- |
| Light | HTTP/2 over TCP | Maximum speed |
| Standard | HTTP/3 over QUIC | Balanced |
| Paranoid | HTTP/2, 60s keepalive | Maximum stealth |

## VPN tab

Runs the tunnel directly inside the app — no config file on disk, no AmneziaWG needed.

On first connect the app registers a fresh anonymous Cloudflare account and stores it in `%APPDATA%\WARP Generator\usque.json`. Every user gets their own account and their own IP.

Settings come from the Generator tab: DNS, MTU and the stealth profile are applied to the tunnel.

Administrator rights are required — the app brings up a TUN adapter and changes routes.

Closing the window hides the app to the tray and the tunnel keeps running. Full disconnect and exit are in the tray menu.

Logs are written to `%TEMP%\warp-gen.log`.

## Project structure

```text
├── src/
│   ├── main.ts
│   ├── styles.css
│   └── assets/
│
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   ├── i1_masks.rs
│   │   ├── quic.rs
│   │   ├── sakeen.rs
│   │   └── vpn.rs
│   ├── binaries/
│   │   ├── usque-x86_64-pc-windows-msvc.exe
│   │   └── wintun.dll
│   ├── capabilities/
│   ├── icons/
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── index.html
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## Configuration

No environment variables needed. The app registers anonymously against the public Cloudflare WARP API.

## Platform support

| Platform | Status |
| --- | --- |
| Windows 10 (2020+) | Works |
| Windows 11 | Works |

Windows only.

## Links

Mine:

- Website: https://warp.sakeen.ru
- GitHub (web): https://github.com/Thiefgg/warp-config-generator-vercel
- GitHub (desktop): https://github.com/Thiefgg/warp-generator-desktop
- Telegram: https://t.me/dar1ysu

Original project:

- Website: https://warp3.llimonix.pw
- GitHub: https://github.com/nellimonix/warp-config-generator-vercel

## License

MIT. See [LICENSE](LICENSE) for the full text.

This fork keeps the original license and attribution.

## Star history

[![Star History Chart](https://api.star-history.com/chart?repos=Thiefgg/warp-generator-desktop&type=date&legend=bottom-right)](https://www.star-history.com/?repos=Thiefgg%2Fwarp-generator-desktop&type=date&legend=bottom-right)

---

Based on the open-source work of [llimonix / nellimonix](https://github.com/nellimonix/warp-config-generator-vercel).

Customized and maintained by [Sakeenkok](https://github.com/Thiefgg).
