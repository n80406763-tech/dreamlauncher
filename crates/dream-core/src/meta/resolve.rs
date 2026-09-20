//! Слияние цепочки `inheritsFrom` в один плоский `ResolvedVersion`.
//!
//! Дочерний JSON (загрузчик) всегда мержится поверх родительского (ваниль):
//! списки конкатенируются с дочерним впереди, скаляры дочернего элемента
//! перекрывают родительские, если заданы.

use super::rules::RuleContext;
use super::version_json::{Arguments, Downloads, JavaVersionRef, Library, MavenCoords, RawVersionJson};
use crate::error::{CoreError, Result};
use std::collections::HashSet;

/// Итог слияния всей цепочки `inheritsFrom` — то, с чем дальше работают
/// модули `download`, `java` и `launch`.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub id: String,
    pub main_class: String,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    pub asset_index: super::version_json::AssetIndexRef,
    pub assets: String,
    pub downloads: Downloads,
    pub java_version: JavaVersionRef,
    /// Уже дедуплицированный список библиотек, в порядке
    /// «загрузчик → ... → ваниль» (порядок важен для classpath).
    pub libraries: Vec<Library>,
    pub compliance_level: u32,
}

/// Слить цепочку JSON, начиная с самого дочернего (`chain[0]`) до корневого
/// ванильного (`chain.last()`). Вызывающий код отвечает за то, чтобы
/// подгрузить всех родителей по `inheritsFrom` до вызова этой функции.
pub fn merge_chain(chain: Vec<RawVersionJson>) -> Result<ResolvedVersion> {
    if chain.is_empty() {
        return Err(CoreError::Other("пустая цепочка версий для слияния".into()));
    }

    let id = chain[0].id.clone();
    let mut main_class = None;
    let mut minecraft_arguments = None;
    let mut merged_args = Arguments::default();
    let mut has_structured_args = false;
    let mut asset_index = None;
    let mut assets = None;
    let mut downloads = Downloads::default();
    let mut java_version = None;
    let mut compliance_level = None;

    let mut libraries = Vec::new();
    let mut seen = HashSet::new();

    // Идём от дочернего к родительскому: первое встреченное значение
    // скаляра побеждает, библиотеки добавляем все (дубликаты по
    // group:artifact отбрасываем — раньше добавленная версия важнее).
    for version in &chain {
        if main_class.is_none() && !version.main_class.is_empty() {
            main_class = Some(version.main_class.clone());
        }
        if minecraft_arguments.is_none() {
            minecraft_arguments = version.minecraft_arguments.clone();
        }
        if let Some(args) = &version.arguments {
            has_structured_args = true;
            merged_args.jvm.extend(args.jvm.iter().cloned());
            merged_args.game.extend(args.game.iter().cloned());
        }
        if asset_index.is_none() {
            asset_index = version.asset_index.clone();
        }
        if assets.is_none() {
            assets = version.assets.clone();
        }
        if downloads.client.is_none() {
            downloads.client = version.downloads.as_ref().and_then(|d| d.client.clone());
        }
        if downloads.server.is_none() {
            downloads.server = version.downloads.as_ref().and_then(|d| d.server.clone());
        }
        if downloads.client_mappings.is_none() {
            downloads.client_mappings =
                version.downloads.as_ref().and_then(|d| d.client_mappings.clone());
        }
        if downloads.server_mappings.is_none() {
            downloads.server_mappings =
                version.downloads.as_ref().and_then(|d| d.server_mappings.clone());
        }
        if java_version.is_none() {
            java_version = version.java_version.clone();
        }
        if compliance_level.is_none() {
            compliance_level = version.compliance_level;
        }

        // Библиотеки внутри ОДНОГО JSON друг с другом не дедуплицируются:
        // Mojang сам иногда перечисляет один и тот же group:artifact
        // дважды с разными `rules`/natives (классический пример — 1.12.2,
        // где отдельно идут lwjgl 2.9.4 "не-osx" и lwjgl 2.9.2 "только
        // osx") — обе записи нужны, `rules` разберутся во время выбора
        // classpath. Дедуп нужен только против версий, уже обработанных
        // раньше в цепочке (то есть более дочерних — версия загрузчика
        // идёт впереди ванильного родителя).
        let mut keys_in_this_version = Vec::with_capacity(version.libraries.len());
        for lib in &version.libraries {
            let key = MavenCoords::parse(&lib.name)
                .map(|c| c.group_artifact())
                .unwrap_or_else(|| lib.name.clone());
            if !seen.contains(&key) {
                libraries.push(lib.clone());
            }
            keys_in_this_version.push(key);
        }
        seen.extend(keys_in_this_version);
    }

    Ok(ResolvedVersion {
        id,
        main_class: main_class
            .ok_or_else(|| CoreError::Other("mainClass отсутствует во всей цепочке".into()))?,
        minecraft_arguments,
        arguments: has_structured_args.then_some(merged_args),
        asset_index: asset_index
            .ok_or_else(|| CoreError::Other("assetIndex отсутствует во всей цепочке".into()))?,
        assets: assets.unwrap_or_else(|| "legacy".to_string()),
        downloads,
        // Версии до 1.17 не публикуют javaVersion — по умолчанию используем
        // jre-legacy (Java 8); окончательный выбор компонента с учётом
        // диапазонов версий делает модуль `java` (M2).
        java_version: java_version.unwrap_or(JavaVersionRef {
            component: "jre-legacy".to_string(),
            major_version: 8,
        }),
        libraries,
        compliance_level: compliance_level.unwrap_or(0),
    })
}

impl ResolvedVersion {
    /// Библиотеки, чьи `rules` разрешают их в данном окружении — то, что
    /// реально нужно скачать и положить в classpath/натив-каталог.
    pub fn applicable_libraries<'a>(&'a self, ctx: &RuleContext) -> Vec<&'a Library> {
        self.libraries
            .iter()
            .filter(|l| super::rules::rules_allow(&l.rules, ctx))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> RawVersionJson {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).expect("fixture must exist");
        serde_json::from_str(&text).expect("fixture must parse")
    }

    #[test]
    fn fabric_child_overrides_vanilla_parent() {
        let fabric = fixture("fabric-1.21.1.json");
        let vanilla = fixture("version-1.21.1.json");
        assert_eq!(fabric.inherits_from.as_deref(), Some("1.21.1"));

        let vanilla_lib_count = vanilla.libraries.len();
        let fabric_lib_count = fabric.libraries.len();

        let resolved = merge_chain(vec![fabric, vanilla]).unwrap();

        // mainClass — от Fabric, не от ванили.
        assert_eq!(resolved.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        // Ассеты/downloads/javaVersion наследуются от родителя.
        assert!(resolved.downloads.client.is_some());
        assert_eq!(resolved.java_version.component, "java-runtime-delta");
        // Библиотеки обеих версий присутствуют (за вычетом дублей).
        assert!(resolved.libraries.len() >= fabric_lib_count);
        assert!(resolved.libraries.len() <= fabric_lib_count + vanilla_lib_count);
    }

    #[test]
    fn single_version_merges_to_itself() {
        let vanilla = fixture("version-1.12.2.json");
        let resolved = merge_chain(vec![vanilla.clone()]).unwrap();
        assert_eq!(resolved.id, vanilla.id);
        assert_eq!(resolved.main_class, vanilla.main_class);
        assert_eq!(resolved.libraries.len(), vanilla.libraries.len());
    }

    #[test]
    fn empty_chain_is_rejected() {
        assert!(merge_chain(vec![]).is_err());
    }
}
