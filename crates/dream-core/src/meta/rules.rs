//! Оценка блоков `rules`, которые Mojang использует и для библиотек, и для
//! аргументов JVM/игры. Логика одна и та же в обоих местах:
//!
//! - Пустой список правил → разрешено.
//! - Непустой список → по умолчанию запрещено, разрешается первым же
//!   подходящим `allow` и может быть отменено последующим `disallow`
//!   (правила применяются по порядку, как в игре).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    /// Regex-строка (как в оригинальных JSON); в 2026 году не встречается
    /// ни у одной активной версии, но оставляем для полноты разбора.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

/// Текущее окружение, относительно которого проверяются правила.
#[derive(Debug, Clone)]
pub struct RuleContext {
    pub os_name: &'static str, // "windows" | "linux" | "osx"
    pub os_arch: &'static str, // "x86" | "x86_64"/"amd64" | "arm64"/"aarch64"
    pub features: HashMap<String, bool>,
}

impl RuleContext {
    /// Контекст для текущей ОС/архитектуры, без активных фич (demo,
    /// custom resolution и т.д.) — их включает вызывающий код при сборке
    /// команды запуска.
    pub fn current() -> Self {
        let os_name = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "osx"
        } else {
            "linux"
        };
        let os_arch = if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86"
        };
        Self { os_name, os_arch, features: HashMap::new() }
    }

    pub fn with_feature(mut self, key: &str, value: bool) -> Self {
        self.features.insert(key.to_string(), value);
        self
    }

    fn os_matches(&self, os: &OsRule) -> bool {
        if let Some(name) = &os.name {
            // Mojang использует "osx" для macOS и, в части старых записей,
            // "windows"/"linux" без учёта регистра — сравниваем как есть.
            if name != self.os_name {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            let normalized = match arch.as_str() {
                "x86" => "x86",
                "x86_64" | "amd64" => "x86_64",
                "arm64" | "aarch64" => "arm64",
                other => other,
            };
            if normalized != self.os_arch {
                return false;
            }
        }
        // `version` — regex по строке версии ОС; ни одна поддерживаемая
        // версия Minecraft на 2026 год его не задаёт, поэтому намеренно
        // не матчим (регекс на "^10\." для Windows 10 давно неактуален
        // и его отсутствие не влияет на результат для актуальных версий).
        true
    }

    fn features_match(&self, required: &HashMap<String, bool>) -> bool {
        required.iter().all(|(k, v)| self.features.get(k).copied().unwrap_or(false) == *v)
    }
}

/// Итог применения списка правил: разрешено ли то, что они охраняют.
pub fn rules_allow(rules: &[Rule], ctx: &RuleContext) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let os_ok = rule.os.as_ref().is_none_or(|os| ctx.os_matches(os));
        let features_ok = rule.features.as_ref().is_none_or(|f| ctx.features_match(f));
        if os_ok && features_ok {
            allowed = matches!(rule.action, RuleAction::Allow);
        }
    }
    allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(os: &'static str, arch: &'static str) -> RuleContext {
        RuleContext { os_name: os, os_arch: arch, features: HashMap::new() }
    }

    #[test]
    fn empty_rules_are_allowed() {
        assert!(rules_allow(&[], &ctx("windows", "x86_64")));
    }

    #[test]
    fn simple_os_allow() {
        let rules: Vec<Rule> = serde_json::from_str(
            r#"[{"action":"allow","os":{"name":"osx"}}]"#,
        )
        .unwrap();
        assert!(rules_allow(&rules, &ctx("osx", "x86_64")));
        assert!(!rules_allow(&rules, &ctx("windows", "x86_64")));
    }

    #[test]
    fn allow_then_disallow_specific_os() {
        // Схема из lwjgl-platform в 1.12.2: разрешено всем, кроме osx.
        let rules: Vec<Rule> = serde_json::from_str(
            r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#,
        )
        .unwrap();
        assert!(rules_allow(&rules, &ctx("windows", "x86_64")));
        assert!(rules_allow(&rules, &ctx("linux", "x86_64")));
        assert!(!rules_allow(&rules, &ctx("osx", "x86_64")));
    }

    #[test]
    fn arch_specific_rule() {
        let rules: Vec<Rule> =
            serde_json::from_str(r#"[{"action":"allow","os":{"arch":"x86"}}]"#).unwrap();
        assert!(rules_allow(&rules, &ctx("windows", "x86")));
        assert!(!rules_allow(&rules, &ctx("windows", "x86_64")));
    }

    #[test]
    fn feature_gated_rule() {
        let rules: Vec<Rule> = serde_json::from_str(
            r#"[{"action":"allow","features":{"is_demo_user":true}}]"#,
        )
        .unwrap();
        assert!(!rules_allow(&rules, &ctx("windows", "x86_64")));
        assert!(rules_allow(&rules, &ctx("windows", "x86_64").with_feature("is_demo_user", true)));
    }
}
