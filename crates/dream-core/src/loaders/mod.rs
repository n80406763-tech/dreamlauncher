//! Загрузчики модов. Fabric/Quilt (`fabric_like`) — реализовано полностью,
//! отдают готовый `<version>.json`. Forge/NeoForge (`forge`) — разобран
//! `install_profile.json` и резолвер токенов; выполнение процессоров
//! патчинга (M6) — впереди.

pub mod fabric_like;
pub mod forge;

pub use fabric_like::FabricLikeKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderKind {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl LoaderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LoaderKind::Vanilla => "vanilla",
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
            LoaderKind::Forge => "forge",
            LoaderKind::NeoForge => "neoforge",
        }
    }

    /// Слаг, которым Modrinth помечает проекты, совместимые с этим
    /// загрузчиком. Quilt дополнительно принимает `fabric`-моды (Quilt
    /// исполняет их через прослойку совместимости).
    pub fn modrinth_loader_facets(self) -> &'static [&'static str] {
        match self {
            LoaderKind::Vanilla => &[],
            LoaderKind::Fabric => &["fabric"],
            LoaderKind::Quilt => &["quilt", "fabric"],
            LoaderKind::Forge => &["forge"],
            LoaderKind::NeoForge => &["neoforge"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quilt_also_accepts_fabric_mods() {
        assert!(LoaderKind::Quilt.modrinth_loader_facets().contains(&"fabric"));
    }
}
