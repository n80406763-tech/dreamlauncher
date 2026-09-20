# Руководство по разработке DreamLauncher

## Структура проекта

### Основные директории

```
DreamLauncher/
├─ crates/dream-core/        # Независимое от Tauri ядро
│  ├─ src/auth/              # Авторизация (Microsoft OAuth, офлайн)
│  ├─ src/meta/              # Метаданные версий Minecraft
│  ├─ src/download/          # Движок загрузки файлов
│  ├─ src/java/              # Управление Java-рантаймами
│  ├─ src/loaders/           # Загрузчики модов
│  ├─ src/launch/            # Запуск игры
│  ├─ src/mods/              # Работа с модами (Modrinth)
│  └─ src/storage/           # SQLite и файловая система
├─ src-tauri/                # IPC-слой Tauri
│  └─ src/commands/          # Команды для фронтенда
├─ src/                      # React-фронтенд
│  ├─ components/            # UI-компоненты
│  ├─ screens/               # Экраны приложения
│  ├─ stores/                # Zustand-стейт
│  └─ ipc/                   # Типизированные биндинги
└─ scripts/                  # Утилиты сборки
```

## Разработка

### Команды разработки

```bash
# Запуск в режиме разработки
npm run tauri dev

# Сборка
npm run build
npm run tauri build

# Линтинг
npm run lint
cargo clippy --workspace --all-targets

# Тесты
cargo test --workspace                           # Unit-тесты
cargo test --workspace -- --ignored --nocapture  # Интеграционные

# Регенерация TypeScript-биндингов
npm run gen:bindings
```

### Добавление новой Tauri-команды

1. **Реализуйте логику в `dream-core`** (если нужно):
   ```rust
   // crates/dream-core/src/your_module.rs
   pub fn your_function() -> Result<YourType> {
       // ...
   }
   ```

2. **Создайте команду в `src-tauri/src/commands/`**:
   ```rust
   #[tauri::command]
   #[specta::specta]
   pub fn your_command(state: tauri::State<AppState>) -> Result<YourDto> {
       // вызов dream_core::your_module::your_function()
   }
   ```

3. **Экспортируйте в `commands/mod.rs`**:
   ```rust
   pub use your_module::your_command;
   ```

4. **Добавьте в `lib.rs`**:
   ```rust
   Builder::<tauri::Wry>::new().commands(collect_commands![
       // ...
       commands::your_module::your_command,
   ])
   ```

5. **Регенерируйте биндинги**:
   ```bash
   npm run gen:bindings
   ```

6. **Используйте во фронтенде**:
   ```typescript
   import { commands } from "./ipc/bindings";
   const result = await commands.yourCommand();
   ```

### Добавление нового экрана

1. Создайте компонент в `src/screens/YourScreen.tsx`
2. Добавьте в роутинг `src/App.tsx`
3. Добавьте в `src/stores/ui.ts` (если нужна навигация)
4. Добавьте пункт в `src/components/Sidebar.tsx`

### Работа с БД

Схема SQLite находится в `crates/dream-core/src/storage/schema.rs`.

Для изменения схемы:
1. Обновите `SCHEMA` константу
2. Создайте миграцию (пока вручную)
3. Обновите тесты в `schema.rs`

## Архитектурные решения

### Типобезопасный IPC

Используется `tauri-specta` для автогенерации TypeScript-типов из Rust-сигнатур.

**Rust:**
```rust
#[tauri::command]
#[specta::specta]
pub fn example(arg: String) -> Result<ExampleDto> { /* ... */ }
```

**TypeScript (автогенерированный):**
```typescript
commands.example: (arg: string) => Promise<Result<ExampleDto, DreamError>>
```

### Обработка ошибок

Все команды возвращают `Result<T, DreamError>`, который маппится в TypeScript как `Result<T, DreamError>`.

**Rust:**
```rust
pub enum CoreError {
    Io(String),
    Network(String),
    Auth(String),
    Other(String),
}
```

**TypeScript:**
```typescript
type DreamError = 
  | { kind: "Io"; message: string }
  | { kind: "Network"; message: string }
  | { kind: "Auth"; message: string }
  | { kind: "Other"; message: string };
```

### Прогресс длительных операций

Используется `tauri::ipc::Channel<T>` для потоковой отправки прогресса:

**Rust:**
```rust
#[tauri::command]
#[specta::specta]
pub async fn long_operation(
    progress: Channel<ProgressEvent>
) -> Result<()> {
    progress.send(ProgressEvent::Started)?;
    // ...
    progress.send(ProgressEvent::Progress { done: 5, total: 10 })?;
    // ...
    progress.send(ProgressEvent::Done)?;
    Ok(())
}
```

**TypeScript:**
```typescript
const channel = new Channel<ProgressEvent>();
channel.onmessage = (event) => {
  // обработка прогресса
};
await commands.longOperation(channel);
```

## Тестирование

### Unit-тесты

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_function() {
        let result = your_function();
        assert!(result.is_ok());
    }
}
```

### Интеграционные тесты

Помечены `#[ignore]` — ходят в сеть, запускаются вручную:

```rust
#[tokio::test]
#[ignore]
async fn test_real_api() {
    let client = reqwest::Client::new();
    // реальный HTTP-запрос
}
```

Запуск:
```bash
cargo test -- --ignored --nocapture
```

### Тестовые фикстуры

Находятся в `crates/dream-core/tests/fixtures/`:
- `version-*.json` — снапшоты реальных version JSON
- `modrinth-*.json` — примеры ответов Modrinth API

## Дизайн-система

### CSS-токены

Все цвета, отступы и тайминги определены в `src/styles/tokens.css`:

```css
:root {
  --bg-void: #0f1114;
  --bg-stone: #1b1f24;
  --text-hi: #f2f2f2;
  --accent: #3c8527;
  --sp-4: 16px;
  --dur: 120ms;
}
```

### Компоненты

**Button:**
```tsx
<Button variant="primary" size="sm" onClick={handleClick}>
  Кнопка
</Button>
```

**Panel:**
```tsx
<Panel raised className="p-4">
  Контент панели
</Panel>
```

**Toast:**
```typescript
import { toast } from "./components/ui/Toast";
toast.success("Операция успешна!");
toast.error("Произошла ошибка");
```

**Dialog:**
```tsx
<Dialog
  isOpen={isOpen}
  onClose={() => setIsOpen(false)}
  title="Заголовок"
  actions={<Button onClick={handleSave}>Сохранить</Button>}
>
  Контент диалога
</Dialog>
```

## Релиз

### Подготовка к релизу

1. Обновите версию в `Cargo.toml` и `package.json`
2. Создайте `CHANGELOG.md` с изменениями
3. Запустите полный набор тестов
4. Соберите релизную версию

### Создание инсталлятора

```bash
npm run tauri build
```

Результат в `src-tauri/target/release/bundle/`:
- `nsis/DreamLauncher_x.x.x_x64-setup.exe` (Windows)
- `msi/DreamLauncher_x.x.x_x64.msi` (Windows)

### Подпись кода

1. Получите сертификат подписи кода
2. Настройте переменные окружения:
   ```bash
   TAURI_SIGNING_PRIVATE_KEY=path/to/key
   TAURI_SIGNING_PASSWORD=your_password
   ```
3. Соберите с подписью

## Troubleshooting

### Ошибка компиляции dream-core

```bash
cargo clean
cargo build --workspace
```

### TypeScript-ошибки в биндингах

```bash
npm run gen:bindings
```

### Проблемы с Windows Credential Manager

Проверьте права доступа к keyring:
```rust
TokenVault::store_refresh_token("test-id", "test-token")?;
```

### Ошибки загрузки файлов

Проверьте:
1. SHA1-суммы в метаданных
2. Доступность URL (может быть заблокирован файрволом)
3. Права на запись в директорию

## FAQ

**Q: Почему не работает Microsoft OAuth?**  
A: Требуется `DREAMLAUNCHER_CLIENT_ID` в переменных окружения. Зарегистрируйте Azure-приложение.

**Q: Как добавить новый загрузчик модов?**  
A: Реализуйте `LoaderKind` в `crates/dream-core/src/loaders/mod.rs` и добавьте установку по аналогии с Fabric.

**Q: Как изменить цветовую схему?**  
A: Отредактируйте CSS-переменные в `src/styles/tokens.css`.

**Q: Где хранятся данные приложения?**  
A: `%APPDATA%\DreamLauncher\` на Windows:
- `launcher.db` — SQLite
- `shared/` — общие файлы (Java, библиотеки, ассеты)
- `instances/` — инстансы с `.minecraft/`

## Ссылки

- [Tauri Docs](https://tauri.app/)
- [React 19 Docs](https://react.dev/)
- [Modrinth API](https://docs.modrinth.com/)
- [Minecraft Version Manifest](https://piston-meta.mojang.com/mc/game/version_manifest_v2.json)
