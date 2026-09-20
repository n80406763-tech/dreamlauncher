# План: Система шейдер-паков для DreamLauncher

## Цель
Создать полноценную систему управления шейдер-паками (визуальными модификациями Minecraft) через Modrinth API с установкой, переключением и настройками.

## Исследование архитектуры

### Существующая инфраструктура
1. **Моды через Modrinth** (`dream-core/src/mods/modrinth.rs`):
   - Поиск с facets (game_version, loader, project_type)
   - Установка с зависимостями
   - Toggle/Remove
   - БД: таблица `installed_mods`

2. **UI паттерны** (`ModsPanel.tsx`):
   - Поиск с дебаунсом
   - Список установленных с toggle/remove
   - Установка по ссылке/slug
   - Пагинация

3. **Структура проекта**:
   - Rust backend: `dream-core/src/`
   - Tauri commands: `src-tauri/src/commands/`
   - React UI: `src/components/` и `src/screens/`
   - TypeScript биндинги: автогенерация через tauri-specta

### Особенности шейдер-паков
1. **Расположение**: `<instance>/.minecraft/shaderpacks/`
2. **Формат**: `.zip` файлы (не распаковываются)
3. **Активация**: только один активный шейдер за раз
4. **Зависимости**: требуют Iris (Fabric/Quilt) или OptiFine (Forge)
5. **Совместимость**: project_type: "shader" в Modrinth API

## Архитектурный подход

### Вариант A: Расширить существующую систему модов (РЕКОМЕНДУЕТСЯ)
**Плюсы**:
- Переиспользование кода (95% логики идентичны)
- Единая таблица БД с полем `content_type`
- Меньше дублирования

**Минусы**:
- Моды и шейдеры смешаны в одной системе
- Нужна миграция БД

### Вариант B: Отдельная параллельная система
**Плюсы**:
- Полная изоляция
- Проще рассуждать о коде

**Минусы**:
- Дублирование 90% кода
- Две таблицы БД
- Больше поддержки

**ВЫБОР: Вариант A** — расширение существующей системы с полем `content_type`.

## Детальный план реализации

### Этап 1: Backend — расширение core-модуля

#### 1.1. Обновить `modrinth.rs`
```rust
// Новая функция для поиска шейдеров
pub async fn search_shaders(
    client: &reqwest::Client,
    query: &str,
    game_version: &str,
    limit: u32,
    offset: u32
) -> Result<SearchResponse>

// Изменить facets_json для поддержки project_type
fn facets_json(project_type: &str, game_version: &str, loader_facets: &[&str]) -> String
```

#### 1.2. Создать новый модуль `shaders.rs`
```rust
// crates/dream-core/src/shaders/mod.rs
pub mod install;
pub mod detect;

// Определение зависимостей шейдеров
pub fn shader_loader_dependency(loader: &str) -> Option<&'static str> {
    match loader {
        "fabric" | "quilt" => Some("iris"),
        "forge" | "neoforge" => Some("optifine"), // или Oculus
        _ => None
    }
}
```

#### 1.3. Миграция схемы БД
```sql
-- Добавить поле content_type в installed_mods
ALTER TABLE installed_mods ADD COLUMN content_type TEXT NOT NULL DEFAULT 'mod' CHECK (content_type IN ('mod', 'shader', 'resourcepack'));

-- Добавить поле active_shader_id для инстанса
ALTER TABLE instances ADD COLUMN active_shader_id TEXT REFERENCES installed_mods(project_id);

-- Переименовать таблицу для универсальности
ALTER TABLE installed_mods RENAME TO installed_content;
```

### Этап 2: IPC-команды

#### 2.1. Новые команды в `src-tauri/src/commands/shaders.rs`
```rust
#[tauri::command]
#[specta::specta]
pub async fn shaders_search(
    state: State<'_, AppState>,
    instance_id: String,
    query: String,
    offset: u32
) -> Result<SearchResultDto>

#[tauri::command]
#[specta::specta]
pub fn shaders_list(
    state: State<'_, AppState>,
    instance_id: String
) -> Result<Vec<InstalledShaderDto>>

#[tauri::command]
#[specta::specta]
pub async fn shaders_install(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String
) -> Result<String>

#[tauri::command]
#[specta::specta]
pub fn shaders_set_active(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: Option<String> // None = отключить шейдеры
) -> Result<()>

#[tauri::command]
#[specta::specta]
pub fn shaders_remove(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String
) -> Result<()>
```

#### 2.2. DTO типы
```rust
#[derive(Serialize, Deserialize, Type)]
pub struct InstalledShaderDto {
    pub project_id: String,
    pub version_id: String,
    pub filename: String,
    pub is_active: bool,
}
```

### Этап 3: Frontend — UI компоненты

#### 3.1. Создать `ShadersPanel.tsx`
Аналогично `ModsPanel.tsx` с отличиями:
- Только один активный шейдер (radio buttons вместо toggles)
- Кнопка "Отключить шейдеры"
- Проверка наличия Iris/OptiFine при установке
- Иконка шейдера в результатах поиска

#### 3.2. Обновить `InstanceSettingsPanel.tsx`
Добавить новую вкладку "Шейдеры" рядом с "Настройки" и "Моды":
```tsx
type InstanceTab = "settings" | "mods" | "shaders";
```

#### 3.3. Hooks для работы с шейдерами
```typescript
// src/hooks/queries.ts
export function useInstalledShaders(instanceId: string, enabled: boolean)
export function useShaderSearch(instanceId: string, query: string, offset: number, enabled: boolean)
export function useInstallShader(instanceId: string)
export function useSetActiveShader(instanceId: string)
export function useRemoveShader(instanceId: string)
```

### Этап 4: Интеграция и тестирование

#### 4.1. Unit-тесты
```rust
// crates/dream-core/src/shaders/tests.rs
#[test]
fn detects_shader_loader_requirements()

#[tokio::test]
async fn searches_shaders_from_modrinth()

#[tokio::test]
#[ignore]
async fn installs_real_shader_pack()
```

#### 4.2. Интеграционные тесты
```rust
// src-tauri/src/commands/shaders.rs
#[tokio::test]
#[ignore]
async fn installs_bsl_shaders_and_sets_active()
```

#### 4.3. E2E сценарий
1. Создать Fabric инстанс
2. Установить Iris (если не установлен)
3. Найти BSL Shaders
4. Установить
5. Активировать
6. Запустить игру
7. Проверить, что шейдеры применились

### Этап 5: Документация и полировка

#### 5.1. Обновить README.md
```markdown
- ✅ **Шейдер-паки**: установка и управление через Modrinth (Iris/OptiFine)
```

#### 5.2. Добавить в CHANGELOG.md
```markdown
### Added
- Система управления шейдер-паками
- Поиск и установка шейдеров с Modrinth
- Переключение активного шейдера
- Автоопределение зависимостей (Iris/OptiFine)
```

## Структура файлов

```
crates/dream-core/src/
├─ shaders/
│  ├─ mod.rs          # Публичный API
│  ├─ install.rs      # Логика установки
│  └─ detect.rs       # Определение зависимостей

src-tauri/src/commands/
└─ shaders.rs         # Tauri команды

src/
├─ components/
│  └─ ShadersPanel.tsx
├─ hooks/
│  └─ queries.ts      # + shader hooks
└─ ipc/
   └─ bindings.ts     # Автогенерация
```

## Миграция данных

### Версия схемы БД
```sql
-- Текущая версия: 1
-- Новая версия: 2

-- settings таблица для версионирования
INSERT INTO settings (key, value) VALUES ('schema_version', '2');

-- Миграция при открытии БД
ALTER TABLE installed_mods RENAME TO installed_content;
ALTER TABLE installed_content ADD COLUMN content_type TEXT NOT NULL DEFAULT 'mod';
ALTER TABLE instances ADD COLUMN active_shader_id TEXT;
```

## Примеры использования

### Установка шейдера
```typescript
const install = useInstallShader(instanceId);
install.mutate("YL57xq9U"); // BSL Shaders project_id
```

### Активация шейдера
```typescript
const setActive = useSetActiveShader(instanceId);
setActive.mutate("YL57xq9U"); // Включить BSL
setActive.mutate(null);        // Отключить все шейдеры
```

### Поиск
```typescript
const search = useShaderSearch(instanceId, "complementary", 0, true);
// Автоматически фильтрует по версии игры инстанса
```

## Риски и митигации

### Риск 1: Конфликт Iris + OptiFine
**Митигация**: При установке проверять loader и блокировать несовместимые комбинации.

### Риск 2: Миграция БД сломает старые данные
**Митигация**: Версионирование схемы + миграции с откатом.

### Риск 3: Пользователь не понимает, почему шейдеры не работают
**Митигация**: UI показывает, что нужен Iris/OptiFine с кнопкой "Установить".

## Оценка трудозатрат

- **Backend (Rust)**: 4-6 часов
  - Миграция БД: 1 час
  - Расширение modrinth.rs: 1 час
  - Новый модуль shaders: 2 часа
  - Tauri команды: 1-2 часа

- **Frontend (React/TS)**: 3-4 часа
  - ShadersPanel компонент: 2 часа
  - Интеграция в UI: 1 час
  - Hooks: 1 час

- **Тестирование**: 2-3 часа
  - Unit тесты: 1 час
  - Интеграционные: 1 час
  - E2E проверка: 1 час

**Итого**: 9-13 часов чистого времени.

## Альтернативные подходы (не выбраны)

### 1. Встроить в существующую систему модов без изменений
**Почему нет**: Моды и шейдеры имеют разную семантику (toggle vs radio, разные папки).

### 2. Сделать отдельный экран "Шейдеры"
**Почему нет**: Шейдеры тесно связаны с инстансом, логичнее вкладка внутри инстанса.

### 3. Не поддерживать OptiFine
**Почему нет**: Многие пользователи Forge все еще используют OptiFine.

## Следующие шаги после реализации

1. **Resource packs**: аналогичная система для пакетов ресурсов
2. **Data packs**: для data packs (только 1.13+)
3. **Модпаки**: импорт `.mrpack` (уже есть базовая поддержка)
4. **Превью шейдеров**: показывать скриншоты из Modrinth

## Вопросы для уточнения

1. **Нужна ли поддержка локальных .zip шейдеров?** (не из Modrinth)
2. **Показывать ли настройки шейдеров?** (обычно редактируются в игре)
3. **Автоустановка Iris при установке первого шейдера?**
