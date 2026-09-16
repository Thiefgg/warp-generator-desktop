# WARP Generator Desktop

**English** | [Русский](README_ru.md)

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

## Features

- WireGuard and AmneziaWG config generation
- AmneziaWG 1.5 with custom I1 domain masking via QUIC handshake
- 8 DNS providers, including ad-blocking and censorship-resistant ones
- 6 endpoint presets plus custom address and random selection
- IPv4/IPv6 support with optional LAN exclusion
- Configurable MTU (1280, 1420, 1380, 1240, 1200) and Persistent Keepalive
- One-click import into AmneziaWG
- Local config history with confirmation dialog before clearing
- Save As with native Windows dialog
- Settings are auto-saved between sessions
- Small footprint (~10 MB) — no bundled Chromium, uses system WebView2

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

The installer will be in `src-tauri/target/release/bundle/`.

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
│   │   └── quic.rs
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
| macOS | Not tested |
| Linux | Not tested |

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
