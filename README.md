# DreamLauncher

Лаунчер Minecraft: Java Edition на базе **Tauri 2 (Rust) + React 19 + TypeScript + Vite + Tailwind 4**.

![Статус](https://img.shields.io/badge/статус-85%25_завершено-green)
![Rust](https://img.shields.io/badge/Rust-1.85+-orange)
![React](https://img.shields.io/badge/React-19-blue)
![TypeScript](https://img.shields.io/badge/TypeScript-6.0-blue)
![Лицензия](https://img.shields.io/badge/лицензия-MIT-blue)

> ⚠️ **Не является продуктом Mojang или Microsoft.**

## 🎮 Возможности

- ✅ **Все загрузчики**: Vanilla, Fabric, Quilt, **Forge**, **NeoForge**
- ✅ **Все версии**: 908+ версий (релизы, снапшоты, old_beta, old_alpha, включая 1.0 и старше)
- ✅ **Microsoft OAuth**: полная авторизация с Device Code Flow
- ✅ **Офлайн-профили**: для одиночной игры и серверов с `online-mode=false`
- ✅ **Автоустановка Java**: скачивание нужной версии Java-рантайма от Mojang
- ✅ **Моды из Modrinth**: поиск, установка с зависимостями, управление
- ✅ **Изоляция инстансов**: отдельные `.minecraft/` папки, общие библиотеки/ассеты
- ✅ **Minecraft-стилизация**: пиксельный шрифт, bevel-кнопки, ступенчатые анимации

## 🏗️ Архитектура

```
crates/dream-core/   → Ядро: метаданные, загрузка, Java, загрузчики модов, запуск
                        Без Tauri — тестируется обычным `cargo test`
src-tauri/           → IPC-слой: Tauri-команды, SQLite, состояние
src/                 → React UI: экраны, дизайн-система, типобезопасные биндинги
```

## 🚀 Быстрый старт

### Требования

- Node.js 18+
- Rust 1.85+
- Windows 10/11 (macOS/Linux — через WSL или нативно)

### Запуск в режиме разработки

```bash
npm install
npm run tauri dev
```

### Тестирование

```bash
# Unit-тесты в dream-core
cargo test --workspace

# Линтеры
cargo clippy --workspace --all-targets
npm run lint

# Интеграционные тесты (ходят в сеть, запускаются вручную)
cargo test --workspace -- --ignored --nocapture
```

### Регенерация TypeScript-биндингов

После изменения сигнатур Tauri-команд:

```bash
npm run gen:bindings
```

## 📦 Сборка

```bash
npm run build
npm run tauri build
```

Инсталлятор появится в `src-tauri/target/release/bundle/`.

## 🔑 Microsoft OAuth

Для работы Microsoft-авторизации требуется:

1. Зарегистрировать **Azure-приложение** (Public client, personal Microsoft accounts)
2. Подать заявку в **Mojang** на доступ к Minecraft API для сторонних лаунчеров
3. Установить `DREAMLAUNCHER_CLIENT_ID` в переменные окружения

До получения CLIENT_ID работают офлайн-профили.

## 🧪 Подтверждённые версии

**Протестировано на реальных клиентах** (интеграционные тесты):

- ✅ Vanilla 1.0, 1.8.9, 1.21.x, последний снапшот
- ✅ Fabric 1.20.1, 1.21.1
- ✅ Quilt 1.21.1
- ✅ Forge 1.12.2, 1.16.5, 1.20.1
- ✅ NeoForge 1.21.1

Все загружают текстуры, звук, логинятся и доходят до главного меню.

## 📊 Прогресс

- ✅ **M0-M7**: Полная функциональность (ванильный запуск, Java, все загрузчики, моды, инстансы)
- ✅ **M3**: Microsoft OAuth ✨ (реализовано полностью)
- 🔄 **M8**: Дизайн-система (~85% — осталась полировка UI, i18n)
- ⏳ **M9**: Крэш-репорты, инсталлятор, автообновление

Подробный статус: [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md)

## 🛠️ Структура проекта

```
.
├─ crates/dream-core/         # Rust-ядро без Tauri
│  ├─ src/auth/               # Microsoft OAuth, офлайн-профили, keyring
│  ├─ src/meta/               # Манифест версий, разбор JSON, правила
│  ├─ src/download/           # Движок загрузки (конкурентность, SHA1, ретраи)
│  ├─ src/java/               # Установка и выбор Java-рантаймов
│  ├─ src/loaders/            # Fabric, Quilt, Forge, NeoForge
│  ├─ src/launch/             # Сборка команды запуска, classpath, аргументы
│  ├─ src/mods/               # Modrinth API, установка модов с зависимостями
│  └─ src/storage/            # SQLite-схема, раскладка путей на диске
├─ src-tauri/                 # IPC-слой и Tauri-команды
│  └─ src/commands/           # Типизированные команды для фронтенда
├─ src/                       # React UI
│  ├─ components/             # Дизайн-система (Button, Panel, Dialog, Toast…)
│  ├─ screens/                # Экраны: Play, Instances, Accounts, Settings
│  ├─ stores/                 # Zustand для UI-состояния
│  ├─ styles/                 # CSS-токены, компоненты, анимации
│  └─ ipc/bindings.ts         # Автогенерированные TypeScript-типы
└─ README.md                  # Этот файл
```

## 🎨 Дизайн-система

- **Шрифт**: [Monocraft](https://github.com/IdreesInc/Monocraft) (SIL OFL 1.1) — пиксельный моноширинный
- **Цвета**: палитра «камень и трава» (`--bg-stone`, `--accent`, `--xp`, …)
- **Анимации**: ступенчатые переходы `steps(4)` / `steps(8)` вместо `ease`
- **Bevel-эффекты**: через `box-shadow` (светлая рамка сверху-слева, тёмная снизу-справа)
- **Доступность**: навигация с клавиатуры, видимый фокус, `prefers-reduced-motion`

Нет ассетов из игрового jar. Все текстуры и стили — собственные.

## 🧩 Зависимости

### Rust

- `tokio` — асинхронный рантайм
- `reqwest` — HTTP-клиент
- `rusqlite` — SQLite
- `serde`/`serde_json` — сериализация
- `keyring` — Windows Credential Manager для секретов
- `tauri-specta` + `specta` — типобезопасный IPC

### TypeScript

- `react` 19 — UI
- `@tanstack/react-query` — кэширование серверного состояния
- `zustand` — UI-стейт
- `@tauri-apps/api` — IPC с Rust

## 📄 Лицензия

MIT

**Шрифт Monocraft** ([IdreesInc/Monocraft](https://github.com/IdreesInc/Monocraft)) — SIL Open Font License 1.1 (текст лицензии: `src/assets/fonts/LICENSE-Monocraft.txt`).

Никакие ассеты из клиента Minecraft не используются. DreamLauncher — независимый сторонний лаунчер и не является продуктом Mojang или Microsoft.

## 🤝 Вклад

Проект находится в активной разработке. Pull request'ы и issue приветствуются.

## 📞 Контакты

Сообщения об ошибках и предложения — через [GitHub Issues](https://github.com/yourusername/DreamLauncher/issues).

---

Сделано с ❤️ для сообщества Minecraft
