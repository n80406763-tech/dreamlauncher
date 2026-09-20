//! Управление шейдер-паками: установка, активация, определение зависимостей.
//! Шейдеры устанавливаются в `<instance>/.minecraft/shaderpacks/` и требуют
//! Iris (Fabric/Quilt) или OptiFine (Forge/NeoForge).

use crate::loaders::LoaderKind;

/// Возвращает project_id зависимости загрузчика шейдеров для данного мод-лоадера.
/// Iris для Fabric/Quilt, OptiFine для Forge/NeoForge.
pub fn shader_loader_dependency(loader: LoaderKind) -> Option<(&'static str, &'static str)> {
    match loader {
        LoaderKind::Fabric | LoaderKind::Quilt => Some(("YL57xq9U", "Iris")), // Iris Shaders
        LoaderKind::Forge | LoaderKind::NeoForge => Some(("H8CaAYZC", "OptiFine")), // OptiFine
        LoaderKind::Vanilla => None,
    }
}

/// Проверяет, установлен ли загрузчик шейдеров в инстансе.
pub fn has_shader_loader(installed_mods: &[String]) -> bool {
    installed_mods.iter().any(|id| id == "YL57xq9U" || id == "H8CaAYZC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_iris_for_fabric() {
        let (id, name) = shader_loader_dependency(LoaderKind::Fabric).unwrap();
        assert_eq!(id, "YL57xq9U");
        assert_eq!(name, "Iris");
    }

    #[test]
    fn returns_optifine_for_forge() {
        let (id, name) = shader_loader_dependency(LoaderKind::Forge).unwrap();
        assert_eq!(id, "H8CaAYZC");
        assert_eq!(name, "OptiFine");
    }

    #[test]
    fn returns_none_for_vanilla() {
        assert!(shader_loader_dependency(LoaderKind::Vanilla).is_none());
    }

    #[test]
    fn detects_installed_shader_loader() {
        let mods = vec!["sodium".to_string(), "YL57xq9U".to_string()];
        assert!(has_shader_loader(&mods));
    }

    #[test]
    fn returns_false_when_no_shader_loader() {
        let mods = vec!["sodium".to_string(), "lithium".to_string()];
        assert!(!has_shader_loader(&mods));
    }
}
