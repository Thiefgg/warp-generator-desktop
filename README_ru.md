# WARP Generator Desktop

**Русский** | [English](README.md)

!!! Может плохо работать с московскими интернет-провайдерами !!!
В следующем обновлении я добавлю реле для обхода блокировки API Cloudflare на уровне интернет-провайдера.

![WARP Generator Desktop](.github/assets/screenshot-generator.png)

Десктопное приложение для генерации конфигураций Cloudflare WARP и AmneziaWG. Написано на Tauri и Rust.

Проект — десктопный форк оригинальной веб-версии с [warp3.llimonix.pw](https://warp3.llimonix.pw) от [nellimonix](https://github.com/nellimonix). Автор оригинала указан внизу.

## Об этом форке

Это мой личный форк с собственным UI, брендингом и фичами. Веб-версия того же проекта:

- Сайт: https://warp.sakeen.ru
- GitHub (веб): https://github.com/Thiefgg/warp-config-generator-vercel
- GitHub (десктоп): https://github.com/Thiefgg/warp-generator-desktop

## Скриншоты

Главный интерфейс:

![Генератор](.github/assets/screenshot-generator.png)

Раскрытые настройки генератора:

![Опции](.github/assets/screenshot-options.png)

После клика по «Открыть в AmneziaWG» — конфиг сохранён, AmneziaWG открыта с инструкцией по импорту:

![Импорт в AmneziaWG](.github/assets/screenshot-amnezia.png)

## Установка

Скачай последний инсталлятор в разделе [Releases](https://github.com/Thiefgg/warp-generator-desktop/releases).

| Файл | Описание |
| --- | --- |
| `WARP Generator_x.x.x_x64-setup.exe` | Установщик NSIS (рекомендуется) |
| `WARP Generator_x.x.x_x64_en-US.msi` | MSI-пакет |

Требуется Windows 10 (2020 или новее) или Windows 11. WebView2 уже встроен в обе.

## Сборка из исходников

Что нужно:

- Node.js 18 или новее
- Rust 1.75 или новее
- [Зависимости Tauri](https://tauri.app/start/prerequisites/) для твоей ОС

```bash
git clone https://github.com/Thiefgg/warp-generator-desktop.git
cd warp-generator-desktop
npm install
npm run tauri dev      # разработка
npm run tauri build    # релизный инсталлятор
```

Инсталлятор появится в папке сборки, путь которой указан в `src-tauri/.cargo/config.toml` (по умолчанию `C:\Users\<юзер>\AppData\Local\cargo-target\warp-generator\release\bundle\`).

## Форматы конфигураций

| Формат | Что это |
| --- | --- |
| WireGuard | Обычный конфиг, работает в любом WireGuard-клиенте |
| AmneziaWG | Параметры обфускации AmneziaWG |
| Sakeen | Свой формат с секцией `[Stealth]` |

### Sakeen

Собственный формат приложения. Отличие от AmneziaWG — домен для маскировки выбирается случайно при каждой генерации из списка разрешённых ресурсов, поэтому паттерн хендшейка не повторяется и его невозможно выучить.

Три профиля:

| Профиль | Транспорт | Назначение |
| --- | --- | --- |
| Light | HTTP/2 поверх TCP | Максимум скорости |
| Standard | HTTP/3 поверх QUIC | Баланс |
| Paranoid | HTTP/2, keepalive 60 с | Максимум скрытности |

## Вкладка VPN

Запускает туннель прямо в приложении — конфиг не нужно сохранять на диск и не нужна AmneziaWG.

При первом подключении приложение анонимно регистрирует новый аккаунт Cloudflare и хранит его в `%APPDATA%\WARP Generator\usque.json`. У каждого пользователя свой аккаунт и свой IP.

Настройки берутся из вкладки «Генератор»: DNS, MTU и профиль маскировки применяются к туннелю.

Требуются права администратора — приложение поднимает TUN-адаптер и меняет маршруты.

Крестик в окне сворачивает приложение в трей, туннель продолжает работать. Полное отключение и выход — через меню трея.

Логи пишутся в `%TEMP%\warp-gen.log`.

## Структура проекта

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

## Конфигурация

Переменные окружения не нужны. Приложение анонимно регистрируется в публичном Cloudflare WARP API.

## Поддерживаемые платформы

| Платформа | Статус |
| --- | --- |
| Windows 10 (2020+) | Работает |
| Windows 11 | Работает |

Только Windows.

## Ссылки

Мои:

- Сайт: https://warp.sakeen.ru
- GitHub (веб): https://github.com/Thiefgg/warp-config-generator-vercel
- GitHub (десктоп): https://github.com/Thiefgg/warp-generator-desktop
- Telegram: https://t.me/dar1ysu

Оригинальный проект:

- Сайт: https://warp3.llimonix.pw
- GitHub: https://github.com/nellimonix/warp-config-generator-vercel

## Лицензия

MIT. Полный текст — в файле [LICENSE](LICENSE).

Форк сохраняет лицензию и attribution оригинального проекта.

## Star history

[![Star History Chart](https://api.star-history.com/chart?repos=Thiefgg/warp-generator-desktop&type=date&legend=bottom-right)](https://www.star-history.com/?repos=Thiefgg%2Fwarp-generator-desktop&type=date&legend=bottom-right)

---

Основано на open-source работе [llimonix / nellimonix](https://github.com/nellimonix/warp-config-generator-vercel).

Кастомизировано и поддерживается [Sakeenkok](https://github.com/Thiefgg).
