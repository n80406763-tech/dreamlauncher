# Changelog

## [Unreleased]

### Added
- ✨ **Microsoft OAuth авторизация** - полная реализация Device Code Flow
  - Xbox Live → XSTS → Minecraft Services
  - Хранение токенов в Windows Credential Manager
  - Автообновление истекших токенов
  - Проверка лицензии Minecraft
  - UI с прогрессом и кодом для авторизации
- 🎮 Поддержка всех 5 загрузчиков: Vanilla, Fabric, Quilt, **Forge**, **NeoForge**
- 🎯 908+ версий Minecraft (включая alpha, beta, 1.0+)
- ☕ Автоустановка Java-рантаймов от Mojang
- 📦 Моды из Modrinth с автоустановкой зависимостей
- 🎨 Minecraft-стилизованный UI с пиксельным шрифтом Monocraft
- 🔔 Toast-уведомления
- 💬 Модальные диалоги
- 📊 Прогресс-бары для длительных операций
- 🎪 Анимации в стиле Minecraft (ступенчатые переходы)

### Implementation Details
- Типобезопасный IPC между Rust и TypeScript через tauri-specta
- SQLite для хранения метаданных инстансов и аккаунтов
- Изоляция `.minecraft/` папок на инстанс
- Общие библиотеки и ассеты для экономии места
- 74 unit-теста + интеграционные тесты против реальных API

### Known Limitations
- Microsoft OAuth требует регистрации Azure-приложения (CLIENT_ID)
- CurseForge-каталог не реализован (нет API-ключа)
- Только тёмная тема
- Windows 10/11 (другие ОС не тестировались)

## [0.1.0] - 2026-09-XX (планируемый релиз)

### Planned
- 🛠️ Крэш-репорты с автоматическим детектом
- 📋 Панель фоновых задач
- 🌐 i18n (русский + английский)
- 📦 NSIS-инсталлятор
- 🔐 Подпись кода
- ♻️ Автообновление через tauri-updater

---

Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.0.0/)
