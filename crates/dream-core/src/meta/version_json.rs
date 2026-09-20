//! Типы для `<version>.json`, как их отдаёт Mojang (и, в том же формате,
//! Fabric/Quilt meta-API). Один формат обслуживает и ваниль, и загрузчики —
//! разница только в наличии `inheritsFrom` и `minecraftArguments` vs
//! `arguments`.

use super::rules::{rules_allow, Rule, RuleContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `value` в условном аргументе — либо один флаг, либо несколько
/// (например `["--width", "${resolution_width}"]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
        }
    }
}

/// Элемент `arguments.game` / `arguments.jvm`: либо голая строка, либо
/// объект с условием применения.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional {
        #[serde(default)]
        rules: Vec<Rule>,
        value: OneOrMany,
    },
}

impl Argument {
    /// Возвращает значения аргумента, если правила разрешают его в данном
    /// контексте.
    pub fn resolve(&self, ctx: &RuleContext) -> Option<Vec<String>> {
        match self {
            Argument::Plain(s) => Some(vec![s.clone()]),
            Argument::Conditional { rules, value } => {
                rules_allow(rules, ctx).then(|| value.clone().into_vec())
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: HashMap<String, Artifact>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Старая схема натив-классификаторов: os -> ключ в `downloads.classifiers`.
    /// Значение может содержать `${arch}`, который нужно подставить.
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
    /// Для legacy-библиотек Forge/NeoForge/Fabric без секции `downloads` —
    /// базовый URL maven-репозитория, откуда собрать ссылку по координатам.
    #[serde(default)]
    pub url: Option<String>,
    /// Fabric-стиль: sha1/size/md5 на верхнем уровне, без `downloads`.
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Координаты maven вида `group:artifact:version[:classifier][@ext]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenCoords {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl MavenCoords {
    pub fn parse(name: &str) -> Option<Self> {
        let (name, extension) = match name.split_once('@') {
            Some((n, ext)) => (n, ext.to_string()),
            None => (name, "jar".to_string()),
        };
        let mut parts = name.split(':');
        let group = parts.next()?.to_string();
        let artifact = parts.next()?.to_string();
        let version = parts.next()?.to_string();
        let classifier = parts.next().map(|s| s.to_string());
        Some(Self { group, artifact, version, classifier, extension })
    }

    /// Относительный путь внутри репозитория maven, например
    /// `net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar`.
    pub fn to_path(&self) -> String {
        let group_path = self.group.replace('.', "/");
        let file = match &self.classifier {
            Some(c) => format!("{}-{}-{}.{}", self.artifact, self.version, c, self.extension),
            None => format!("{}-{}.{}", self.artifact, self.version, self.extension),
        };
        format!("{group_path}/{}/{}/{file}", self.artifact, self.version)
    }

    /// Ключ вида `group:artifact`, по которому дедуплицируются библиотеки
    /// при слиянии версий — побеждает первое вхождение (версия загрузчика
    /// идёт перед ванильной, т.к. дочерний JSON мержится первым).
    pub fn group_artifact(&self) -> String {
        format!("{}:{}", self.group, self.artifact)
    }
}

/// Артефакт библиотеки, готовый к скачиванию: URL, путь назначения
/// относительно `shared/libraries/`, ожидаемый sha1 (если известен).
#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    pub url: String,
    pub relative_path: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

impl Library {
    /// Основной артефакт (без natives) — то, что идёт в classpath.
    pub fn main_artifact(&self) -> Option<ResolvedArtifact> {
        if let Some(dl) = &self.downloads {
            if let Some(a) = &dl.artifact {
                let path = a
                    .path
                    .clone()
                    .or_else(|| MavenCoords::parse(&self.name).map(|c| c.to_path()))?;
                return Some(ResolvedArtifact {
                    url: a.url.clone(),
                    relative_path: path,
                    sha1: a.sha1.clone(),
                    size: a.size,
                });
            }
            // Есть `downloads`, но без `artifact` (например только
            // classifiers для чисто-нативной библиотеки) — не является
            // classpath-артефактом.
            return None;
        }
        // Нет `downloads` вовсе — legacy/Fabric схема: строим URL из maven координат.
        let coords = MavenCoords::parse(&self.name)?;
        let base = self.url.clone().unwrap_or_else(|| "https://libraries.minecraft.net/".to_string());
        let base = if base.ends_with('/') { base } else { format!("{base}/") };
        let path = coords.to_path();
        Some(ResolvedArtifact {
            url: format!("{base}{path}"),
            relative_path: path,
            sha1: self.sha1.clone(),
            size: self.size,
        })
    }

    /// Натив для текущей ОС, если библиотека его несёт (в любой из двух схем).
    pub fn native_artifact(&self, ctx: &RuleContext) -> Option<ResolvedArtifact> {
        // Новая схема: coords сами содержат классификатор natives-<os>.
        if let Some(coords) = MavenCoords::parse(&self.name) {
            if let Some(classifier) = &coords.classifier {
                if classifier.starts_with("natives-") {
                    return self.main_artifact();
                }
            }
        }
        // Старая схема: `natives` map + `downloads.classifiers`.
        let key_template = self.natives.as_ref()?.get(ctx.os_name)?;
        let key = key_template.replace("${arch}", arch_bits(ctx.os_arch));
        let dl = self.downloads.as_ref()?;
        let artifact = dl.classifiers.get(&key)?;
        Some(ResolvedArtifact {
            url: artifact.url.clone(),
            relative_path: artifact.path.clone().unwrap_or_else(|| format!("{}-{}.jar", self.name, key)),
            sha1: artifact.sha1.clone(),
            size: artifact.size,
        })
    }
}

fn arch_bits(arch: &str) -> &'static str {
    match arch {
        "x86" => "32",
        _ => "64",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default)]
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadEntry {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Downloads {
    pub client: Option<DownloadEntry>,
    pub server: Option<DownloadEntry>,
    pub client_mappings: Option<DownloadEntry>,
    pub server_mappings: Option<DownloadEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionRef {
    pub component: String,
    pub major_version: u32,
}

/// Версия JSON «как есть», без слияния с родителем. Именно в этом виде
/// её отдают piston-meta и meta.fabricmc.net/meta.quiltmc.org.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawVersionJson {
    pub id: String,
    #[serde(rename = "type", default)]
    pub release_type: String,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub main_class: String,
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: Option<Downloads>,
    #[serde(default)]
    pub java_version: Option<JavaVersionRef>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub minimum_launcher_version: Option<u32>,
    #[serde(default)]
    pub compliance_level: Option<u32>,
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub release_time: Option<String>,
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
    fn parses_modern_version_with_structured_arguments() {
        let v = fixture("version-1.21.1.json");
        assert_eq!(v.id, "1.21.1");
        assert!(v.arguments.is_some());
        assert!(v.minecraft_arguments.is_none());
        assert_eq!(v.java_version.unwrap().component, "java-runtime-delta");
        assert!(v.downloads.unwrap().client.is_some());
    }

    #[test]
    fn parses_legacy_version_with_flat_arguments() {
        let v = fixture("version-1.7.10.json");
        assert_eq!(v.id, "1.7.10");
        assert!(v.minecraft_arguments.is_some());
        assert!(v.minecraft_arguments.unwrap().contains("--accessToken"));
    }

    #[test]
    fn resolves_modern_classifier_style_native() {
        let v = fixture("version-1.21.1.json");
        let ctx = RuleContext { os_name: "linux", os_arch: "x86_64", features: Default::default() };
        let lib = v
            .libraries
            .iter()
            .find(|l| l.name.starts_with("org.lwjgl:lwjgl-freetype") && l.name.contains("natives-linux"))
            .expect("fixture should contain a linux native lwjgl-freetype artifact");
        let native = lib.native_artifact(&ctx).expect("must resolve as native");
        assert!(native.relative_path.contains("natives-linux"));
    }

    #[test]
    fn resolves_legacy_classifiers_style_native() {
        let v = fixture("version-1.12.2.json");
        let ctx = RuleContext { os_name: "windows", os_arch: "x86_64", features: Default::default() };
        let lib = v
            .libraries
            .iter()
            .find(|l| l.name == "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209")
            .expect("fixture must contain lwjgl-platform");
        let native = lib.native_artifact(&ctx).expect("must resolve windows native");
        assert!(native.url.ends_with("natives-windows.jar"));
    }

    #[test]
    fn maven_coords_round_trip() {
        let c = MavenCoords::parse("net.fabricmc:fabric-loader:0.16.9").unwrap();
        assert_eq!(c.to_path(), "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
        assert_eq!(c.group_artifact(), "net.fabricmc:fabric-loader");

        let with_classifier = MavenCoords::parse("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap();
        assert_eq!(with_classifier.to_path(), "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar");
    }
}
