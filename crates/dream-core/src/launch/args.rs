//! Подстановка `${...}`-токенов и сборка итоговых списков JVM/game
//! аргументов из `ResolvedVersion`. Не знает ничего про процессы или
//! файловую систему — чистая функция от данных, поэтому легко тестируется.

use crate::meta::{ResolvedVersion, RuleContext};
use std::collections::HashMap;
use std::path::PathBuf;

/// Тип учётной записи, под которой запускается игра — влияет на
/// подстановку `${user_type}` и на то, какой вид имеет `${auth_xuid}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountKind {
    Microsoft,
    Offline,
}

impl AccountKind {
    fn user_type(self) -> &'static str {
        match self {
            AccountKind::Microsoft => "msa",
            AccountKind::Offline => "legacy",
        }
    }
}

/// Параметры «быстрого входа» (`--quickPlay*`), доступные начиная с 1.20.
#[derive(Debug, Clone)]
pub enum QuickPlay {
    Multiplayer { host: String, port: u16 },
    Singleplayer { world: String },
}

/// Всё, что нужно для подстановки токенов в шаблонах аргументов.
#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub player_name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: Option<String>,
    pub account_kind: AccountKind,
    pub version_name: String,
    pub version_type: String,
    pub game_directory: PathBuf,
    pub assets_root: PathBuf,
    pub assets_index_name: String,
    pub natives_directory: PathBuf,
    pub classpath: String,
    pub library_directory: PathBuf,
    pub launcher_name: String,
    pub launcher_version: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quick_play: Option<QuickPlay>,
    pub demo: bool,
}

impl LaunchContext {
    /// Windows использует `;` как разделитель classpath, остальные ОС — `:`.
    fn classpath_separator() -> &'static str {
        if cfg!(target_os = "windows") {
            ";"
        } else {
            ":"
        }
    }

    fn token_map(&self) -> HashMap<&'static str, String> {
        let mut m = HashMap::new();
        m.insert("auth_player_name", self.player_name.clone());
        m.insert("auth_uuid", self.uuid.clone());
        m.insert("auth_access_token", self.access_token.clone());
        // Версии до ~1.7.10 используют `${auth_session}` вместо
        // `${auth_access_token}` — реальный формат исторической сессии
        // Yggdrasil (`token:<accessToken>:<uuid>`), но клиент не
        // валидирует его сам в одиночной игре, так что для офлайна и
        // просто заглушки хватило бы; собираем "по-настоящему" на случай
        // будущей Microsoft-авторизации на этих же версиях.
        m.insert("auth_session", format!("token:{}:{}", self.access_token, self.uuid));
        m.insert("auth_xuid", self.xuid.clone().unwrap_or_default());
        m.insert("user_type", self.account_kind.user_type().to_string());
        m.insert("user_properties", "{}".to_string());
        m.insert("clientid", String::new());
        m.insert("version_name", self.version_name.clone());
        m.insert("version_type", self.version_type.clone());
        m.insert("game_directory", path_string(&self.game_directory));
        m.insert("game_assets", path_string(&self.assets_root));
        m.insert("assets_root", path_string(&self.assets_root));
        m.insert("assets_index_name", self.assets_index_name.clone());
        m.insert("natives_directory", path_string(&self.natives_directory));
        m.insert("classpath", self.classpath.clone());
        m.insert("classpath_separator", Self::classpath_separator().to_string());
        m.insert("library_directory", path_string(&self.library_directory));
        m.insert("launcher_name", self.launcher_name.clone());
        m.insert("launcher_version", self.launcher_version.clone());
        m.insert("resolution_width", self.width.map(|w| w.to_string()).unwrap_or_default());
        m.insert("resolution_height", self.height.map(|h| h.to_string()).unwrap_or_default());
        m
    }

    /// Контекст `rules` (демо-режим, свой размер окна, quickplay), который
    /// определяет, какие условные аргументы попадут в итоговую команду.
    pub fn rule_context(&self) -> RuleContext {
        let mut ctx = RuleContext::current();
        ctx.features.insert("is_demo_user".into(), self.demo);
        ctx.features.insert("has_custom_resolution".into(), self.width.is_some() && self.height.is_some());
        let (has_sp, has_mp) = match &self.quick_play {
            Some(QuickPlay::Singleplayer { .. }) => (true, false),
            Some(QuickPlay::Multiplayer { .. }) => (false, true),
            None => (false, false),
        };
        ctx.features.insert("is_quick_play_singleplayer".into(), has_sp);
        ctx.features.insert("is_quick_play_multiplayer".into(), has_mp);
        ctx.features.insert("has_quick_plays_support".into(), has_sp || has_mp);
        ctx
    }
}

fn path_string(p: &std::path::Path) -> String {
    dunce::simplified(p).to_string_lossy().into_owned()
}

/// Заменяет все `${token}` в строке на значения из карты; неизвестные
/// токены оставляет как есть — так проще диагностировать пропущенный
/// случай, чем молча стереть плейсхолдер.
fn substitute(template: &str, tokens: &HashMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            let key = &after[..end];
            match tokens.get(key) {
                Some(value) => out.push_str(value),
                None => {
                    out.push_str("${");
                    out.push_str(key);
                    out.push('}');
                }
            }
            rest = &after[end + 1..];
        } else {
            out.push_str("${");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Аргументы JVM, которые старый (до `arguments.jvm`) формат вообще не
/// описывает в JSON — реальный официальный лаунчер добавляет их сам.
fn default_legacy_jvm_args() -> Vec<&'static str> {
    vec!["-Djava.library.path=${natives_directory}", "-Dminecraft.launcher.brand=${launcher_name}", "-Dminecraft.launcher.version=${launcher_version}", "-cp", "${classpath}"]
}

/// Собирает полный список JVM-аргументов: JSON-аргументы (или их legacy-
/// эквивалент) + пользовательские `-Xmx`/`-Xms`/доп. флаги.
pub fn build_jvm_args(
    resolved: &ResolvedVersion,
    ctx: &LaunchContext,
    min_ram_mb: u32,
    max_ram_mb: u32,
    extra_user_args: &[String],
) -> Vec<String> {
    let rule_ctx = ctx.rule_context();
    let tokens = ctx.token_map();
    let mut out = Vec::new();

    match &resolved.arguments {
        Some(args) => {
            for arg in &args.jvm {
                if let Some(values) = arg.resolve(&rule_ctx) {
                    out.extend(values.into_iter().map(|v| substitute(&v, &tokens)));
                }
            }
        }
        None => {
            out.extend(default_legacy_jvm_args().into_iter().map(|v| substitute(v, &tokens)));
        }
    }

    out.push(format!("-Xms{min_ram_mb}M"));
    out.push(format!("-Xmx{max_ram_mb}M"));
    out.extend(extra_user_args.iter().cloned());
    out
}

/// Собирает полный список игровых аргументов (после mainClass), из
/// современного `arguments.game` либо из плоской строки `minecraftArguments`.
pub fn build_game_args(resolved: &ResolvedVersion, ctx: &LaunchContext) -> Vec<String> {
    let rule_ctx = ctx.rule_context();
    let tokens = ctx.token_map();
    let mut out = Vec::new();

    match (&resolved.arguments, &resolved.minecraft_arguments) {
        (Some(args), _) => {
            for arg in &args.game {
                if let Some(values) = arg.resolve(&rule_ctx) {
                    out.extend(values.into_iter().map(|v| substitute(&v, &tokens)));
                }
            }
            if let (Some(w), Some(h)) = (ctx.width, ctx.height) {
                if !out.iter().any(|a| a == "--width") {
                    out.push("--width".into());
                    out.push(w.to_string());
                    out.push("--height".into());
                    out.push(h.to_string());
                }
            }
        }
        (None, Some(flat)) => {
            out.extend(flat.split_whitespace().map(|tok| substitute(tok, &tokens)));
            if let (Some(w), Some(h)) = (ctx.width, ctx.height) {
                out.push("--width".into());
                out.push(w.to_string());
                out.push("--height".into());
                out.push(h.to_string());
            }
        }
        (None, None) => {}
    }

    match &ctx.quick_play {
        Some(QuickPlay::Multiplayer { host, port }) => {
            out.push("--quickPlayMultiplayer".into());
            out.push(format!("{host}:{port}"));
        }
        Some(QuickPlay::Singleplayer { world }) => {
            out.push("--quickPlaySingleplayer".into());
            out.push(world.clone());
        }
        None => {}
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::merge_chain;

    fn resolved_fixture(name: &str) -> ResolvedVersion {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let raw = serde_json::from_str(&text).unwrap();
        merge_chain(vec![raw]).unwrap()
    }

    fn base_ctx() -> LaunchContext {
        LaunchContext {
            player_name: "Steve".into(),
            uuid: "11111111-2222-3333-4444-555555555555".into(),
            access_token: "token".into(),
            xuid: None,
            account_kind: AccountKind::Offline,
            version_name: "1.21.1".into(),
            version_type: "release".into(),
            game_directory: PathBuf::from(r"C:\instances\demo\.minecraft"),
            assets_root: PathBuf::from(r"C:\shared\assets"),
            assets_index_name: "17".into(),
            natives_directory: PathBuf::from(r"C:\shared\versions\1.21.1\natives-windows-x64"),
            classpath: "a.jar;b.jar".into(),
            library_directory: PathBuf::from(r"C:\shared\libraries"),
            launcher_name: "DreamLauncher".into(),
            launcher_version: "0.1.0".into(),
            width: None,
            height: None,
            quick_play: None,
            demo: false,
        }
    }

    #[test]
    fn substitutes_known_tokens() {
        let ctx = base_ctx();
        let tokens = ctx.token_map();
        assert_eq!(substitute("${auth_player_name}", &tokens), "Steve");
        assert_eq!(substitute("--cp=${classpath}", &tokens), "--cp=a.jar;b.jar");
        assert_eq!(substitute("${unknown_token}", &tokens), "${unknown_token}");
    }

    #[test]
    fn modern_jvm_args_include_classpath_and_natives() {
        let resolved = resolved_fixture("version-1.21.1.json");
        let ctx = base_ctx();
        let jvm = build_jvm_args(&resolved, &ctx, 1024, 4096, &["-Dfoo=bar".to_string()]);

        assert!(jvm.iter().any(|a| a.contains("natives-windows-x64")));
        assert!(jvm.windows(2).any(|w| w[0] == "-cp" && w[1] == "a.jar;b.jar"));
        assert!(jvm.contains(&"-Xms1024M".to_string()));
        assert!(jvm.contains(&"-Xmx4096M".to_string()));
        assert!(jvm.contains(&"-Dfoo=bar".to_string()));
        // Правило "allow os=windows" должно сработать на реальной сборке
        // (тест гоняется на Windows CI/машине разработчика).
        if cfg!(target_os = "windows") {
            assert!(jvm.iter().any(|a| a.contains("HeapDumpPath")));
        }
    }

    #[test]
    fn legacy_version_synthesizes_jvm_args() {
        let resolved = resolved_fixture("version-1.7.10.json");
        let ctx = base_ctx();
        let jvm = build_jvm_args(&resolved, &ctx, 512, 2048, &[]);
        assert!(jvm.windows(2).any(|w| w[0] == "-cp" && w[1] == "a.jar;b.jar"));
        assert!(jvm.iter().any(|a| a.starts_with("-Djava.library.path=")));
    }

    #[test]
    fn legacy_game_args_split_flat_string() {
        let resolved = resolved_fixture("version-1.7.10.json");
        let mut ctx = base_ctx();
        ctx.version_name = "1.7.10".into();
        let game = build_game_args(&resolved, &ctx);
        assert!(game.windows(2).any(|w| w[0] == "--username" && w[1] == "Steve"));
        assert!(game.windows(2).any(|w| w[0] == "--accessToken" && w[1] == "token"));
    }

    #[test]
    fn modern_game_args_respect_demo_feature() {
        let resolved = resolved_fixture("version-1.21.1.json");
        let mut ctx = base_ctx();
        ctx.demo = true;
        let game = build_game_args(&resolved, &ctx);
        assert!(game.contains(&"--demo".to_string()));

        ctx.demo = false;
        let game = build_game_args(&resolved, &ctx);
        assert!(!game.contains(&"--demo".to_string()));
    }

    #[test]
    fn quick_play_multiplayer_appends_flag() {
        let resolved = resolved_fixture("version-1.21.1.json");
        let mut ctx = base_ctx();
        ctx.quick_play = Some(QuickPlay::Multiplayer { host: "play.example.com".into(), port: 25565 });
        let game = build_game_args(&resolved, &ctx);
        assert!(game.windows(2).any(|w| w[0] == "--quickPlayMultiplayer" && w[1] == "play.example.com:25565"));
    }
}
