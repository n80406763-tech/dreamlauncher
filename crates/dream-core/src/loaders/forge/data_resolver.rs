//! Резолвер `data`-секции и `{TOKEN}`-подстановок в аргументах процессоров
//! Forge/NeoForge. Чистая логика без файлового ввода-вывода — фактическое
//! извлечение файлов из installer-jar и запуск процессоров (M6) строятся
//! поверх этих функций.
//!
//! Каждое значение в `data` — это, для конкретной стороны (client/server),
//! одно из трёх:
//! - `[group:artifact:version[:classifier]@ext]` — координаты maven,
//!   резолвятся в путь библиотеки;
//! - `'литеральная строка'` — используется как есть, без кавычек;
//! - произвольный путь вида `/data/client.lzma` — файл внутри самого
//!   installer-jar, который нужно извлечь во временное место.

use crate::meta::version_json::MavenCoords;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataValue {
    Maven(MavenCoords),
    Literal(String),
    InstallerPath(String),
}

pub fn parse_data_value(raw: &str) -> DataValue {
    if let Some(inner) = raw.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        if let Some(coords) = MavenCoords::parse(inner) {
            return DataValue::Maven(coords);
        }
    }
    if let Some(inner) = raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return DataValue::Literal(inner.to_string());
    }
    DataValue::InstallerPath(raw.to_string())
}

/// Окружение, в котором резолвятся токены аргументов процессора.
pub struct ProcessorContext {
    pub root: PathBuf,
    pub installer: PathBuf,
    pub minecraft_jar: PathBuf,
    pub minecraft_version: String,
    pub library_dir: PathBuf,
    /// Уже резолвленные значения `data` для стороны client: ключ без
    /// фигурных скобок (например `BINPATCH`) -> готовая строка (абсолютный
    /// путь к файлу или литерал).
    pub data: HashMap<String, String>,
}

/// Резолвит один `{TOKEN}` (без фигурных скобок) во встроенные и
/// пользовательские (`data`) значения. Отсутствующий токен — ошибка
/// вызывающего кода, а не тихая заглушка: пропущенный `data`-ключ ломает
/// патчинг незаметно, если его не заметить сразу.
fn resolve_builtin(token: &str, ctx: &ProcessorContext) -> Option<String> {
    match token {
        "SIDE" => Some("client".to_string()),
        "ROOT" => Some(ctx.root.to_string_lossy().into_owned()),
        "INSTALLER" => Some(ctx.installer.to_string_lossy().into_owned()),
        "MINECRAFT_JAR" => Some(ctx.minecraft_jar.to_string_lossy().into_owned()),
        "MINECRAFT_VERSION" => Some(ctx.minecraft_version.clone()),
        "LIBRARY_DIR" => Some(ctx.library_dir.to_string_lossy().into_owned()),
        _ => ctx.data.get(token).cloned(),
    }
}

/// Подставляет все `{TOKEN}` в строке аргумента. В отличие от `${...}` у
/// game/jvm-аргументов, здесь используются одинарные фигурные скобки.
pub fn substitute(arg: &str, ctx: &ProcessorContext) -> Result<String, String> {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return Err(format!("незакрытая '{{' в аргументе процессора: {arg}"));
        };
        let token = &after[..end];
        let value = resolve_builtin(token, ctx).ok_or_else(|| format!("не удалось резолвить токен {{{token}}} в аргументе: {arg}"))?;
        out.push_str(&value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Путь библиотеки для maven-координат внутри `library_dir`.
pub fn maven_path(coords: &MavenCoords, library_dir: &std::path::Path) -> PathBuf {
    library_dir.join(coords.to_path().replace('/', std::path::MAIN_SEPARATOR_STR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maven_data_value() {
        let value = parse_data_value("[de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412:mappings@txt]");
        match value {
            DataValue::Maven(coords) => {
                assert_eq!(coords.group, "de.oceanlabs.mcp");
                assert_eq!(coords.artifact, "mcp_config");
                assert_eq!(coords.classifier.as_deref(), Some("mappings"));
                assert_eq!(coords.extension, "txt");
            }
            other => panic!("ожидали Maven, получили {other:?}"),
        }
    }

    #[test]
    fn parses_literal_data_value() {
        assert_eq!(parse_data_value("'de86b035d2da0f78940796bb95c39a932ed84834'"), DataValue::Literal("de86b035d2da0f78940796bb95c39a932ed84834".to_string()));
    }

    #[test]
    fn parses_installer_path_data_value() {
        assert_eq!(parse_data_value("/data/client.lzma"), DataValue::InstallerPath("/data/client.lzma".to_string()));
    }

    fn ctx() -> ProcessorContext {
        let mut data = HashMap::new();
        data.insert("BINPATCH".to_string(), r"C:\tmp\extracted\client.lzma".to_string());
        ProcessorContext {
            root: PathBuf::from(r"C:\instances\demo"),
            installer: PathBuf::from(r"C:\cache\forge-installer.jar"),
            minecraft_jar: PathBuf::from(r"C:\shared\versions\1.20.1\1.20.1.jar"),
            minecraft_version: "1.20.1".into(),
            library_dir: PathBuf::from(r"C:\shared\libraries"),
            data,
        }
    }

    #[test]
    fn substitutes_builtin_tokens() {
        let ctx = ctx();
        let out = substitute("{ROOT}/run.sh", &ctx).unwrap();
        assert_eq!(out, r"C:\instances\demo/run.sh");
        assert_eq!(substitute("{SIDE}", &ctx).unwrap(), "client");
        assert_eq!(substitute("{MINECRAFT_VERSION}", &ctx).unwrap(), "1.20.1");
    }

    #[test]
    fn substitutes_data_token() {
        let ctx = ctx();
        assert_eq!(substitute("{BINPATCH}", &ctx).unwrap(), r"C:\tmp\extracted\client.lzma");
    }

    #[test]
    fn unknown_token_is_an_explicit_error() {
        let ctx = ctx();
        assert!(substitute("{NOT_A_REAL_TOKEN}", &ctx).is_err());
    }

    #[test]
    fn maven_path_joins_library_dir() {
        let coords = MavenCoords::parse("net.minecraftforge:installertools:1.4.1").unwrap();
        let path = maven_path(&coords, std::path::Path::new(r"C:\shared\libraries"));
        assert_eq!(path, PathBuf::from(r"C:\shared\libraries\net\minecraftforge\installertools\1.4.1\installertools-1.4.1.jar"));
    }
}
