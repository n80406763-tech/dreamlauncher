//! Клиент Modrinth API v2: поиск с автофильтром по версии/загрузчику
//! инстанса и список версий проекта для установки. Схемы полей сверены
//! с реальными ответами `api.modrinth.com` (см. `tests/fixtures/modrinth-*`).

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

pub const BASE_URL: &str = "https://api.modrinth.com/v2";

/// Обязателен по правилам Modrinth: без внятного `User-Agent` запросы
/// могут получать более жёсткий рейт-лимит.
pub fn user_agent(app_version: &str) -> String {
    format!("DreamLauncher/{app_version} (github.com/dreamlauncher; contact: c2@netrender.org)")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub categories: Vec<String>,
    pub versions: Vec<String>,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub latest_version: Option<String>,
    pub client_side: String,
    pub server_side: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    pub hashes: FileHashes,
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(rename = "version_type")]
    pub kind: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl ProjectVersion {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

/// Одна facet-группа — элементы внутри группы соединяются через OR,
/// группы между собой — через AND. Ровно то, что нужно для «версия X
/// И загрузчик Y».
fn facets_json(project_type: &str, game_version: &str, loader_facets: &[&str]) -> String {
    let loader_group: Vec<String> = loader_facets.iter().map(|l| format!("\"categories:{l}\"")).collect();
    format!(
        "[[\"project_type:{project_type}\"],[\"versions:{game_version}\"],[{}]]",
        loader_group.join(",")
    )
}

/// Поиск модов, уже отфильтрованный под конкретный инстанс — версию игры
/// и допустимые загрузчики (для Quilt это `["quilt", "fabric"]`).
pub async fn search(client: &reqwest::Client, query: &str, game_version: &str, loader_facets: &[&str], limit: u32, offset: u32) -> Result<SearchResponse> {
    search_at(client, BASE_URL, query, game_version, loader_facets, limit, offset).await
}

/// Поиск шейдер-паков для конкретной версии игры (загрузчик не важен).
pub async fn search_shaders(client: &reqwest::Client, query: &str, game_version: &str, limit: u32, offset: u32) -> Result<SearchResponse> {
    search_by_type(client, BASE_URL, query, game_version, "shader", &[], limit, offset).await
}

/// То же самое, но с настраиваемым базовым URL — production-код всегда
/// зовёт `search()`, а тесты подставляют сюда адрес `wiremock`-сервера.
pub async fn search_at(client: &reqwest::Client, base_url: &str, query: &str, game_version: &str, loader_facets: &[&str], limit: u32, offset: u32) -> Result<SearchResponse> {
    search_by_type(client, base_url, query, game_version, "mod", loader_facets, limit, offset).await
}

/// Общий поиск с указанием типа проекта (mod, shader, resourcepack).
async fn search_by_type(client: &reqwest::Client, base_url: &str, query: &str, game_version: &str, project_type: &str, loader_facets: &[&str], limit: u32, offset: u32) -> Result<SearchResponse> {
    let facets = facets_json(project_type, game_version, loader_facets);
    let url = format!("{base_url}/search");
    let limit_str = limit.to_string();
    let offset_str = offset.to_string();
    let response = client
        .get(&url)
        .query(&[("query", query), ("facets", facets.as_str()), ("limit", limit_str.as_str()), ("offset", offset_str.as_str()), ("index", "relevance")])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    serde_json::from_str(&response).map_err(|e| CoreError::json(url, e))
}

/// Версии проекта, совместимые с загрузчиком(ами) и версией игры;
/// Modrinth уже сам фильтрует по этим параметрам на своей стороне.
pub async fn list_versions(client: &reqwest::Client, project_id: &str, loader_facets: &[&str], game_version: &str) -> Result<Vec<ProjectVersion>> {
    let loaders_json = format!("[{}]", loader_facets.iter().map(|l| format!("\"{l}\"")).collect::<Vec<_>>().join(","));
    let game_versions_json = format!("[\"{game_version}\"]");
    let url = format!("{BASE_URL}/project/{project_id}/version");
    let response = client
        .get(&url)
        .query(&[("loaders", &loaders_json), ("game_versions", &game_versions_json)])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    serde_json::from_str(&response).map_err(|e| CoreError::json(url, e))
}

/// Первая версия с типом `release`, иначе — просто самая свежая
/// (список от Modrinth уже отсортирован от новых к старым).
pub fn pick_best_version(versions: &[ProjectVersion]) -> Option<&ProjectVersion> {
    versions.iter().find(|v| v.kind == "release").or_else(|| versions.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parses_real_search_response_fixture() {
        let path = format!("{}/tests/fixtures/modrinth-search-sodium.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let parsed: SearchResponse = serde_json::from_str(&text).unwrap();
        assert!(parsed.total_hits > 0);
        assert_eq!(parsed.hits[0].slug, "sodium");
    }

    #[test]
    fn parses_real_version_list_fixture_with_dependencies() {
        let path = format!("{}/tests/fixtures/modrinth-versions-waystones.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let parsed: Vec<ProjectVersion> = serde_json::from_str(&text).unwrap();
        assert!(!parsed.is_empty());
        assert!(parsed[0].dependencies.iter().any(|d| d.dependency_type == DependencyType::Required));

        let best = pick_best_version(&parsed).unwrap();
        assert!(!best.files.is_empty());
        assert!(best.primary_file().is_some());
    }

    #[test]
    fn facets_group_loaders_with_or_and_dimensions_with_and() {
        let facets = facets_json("mod", "1.21.1", &["quilt", "fabric"]);
        assert_eq!(facets, r#"[["project_type:mod"],["versions:1.21.1"],["categories:quilt","categories:fabric"]]"#);
    }

    #[tokio::test]
    async fn search_sends_expected_query_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("query", "sodium"))
            .and(query_param("facets", r#"[["project_type:mod"],["versions:1.21.1"],["categories:fabric"]]"#))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"hits":[],"offset":0,"limit":20,"total_hits":0}"#))
            .expect(1)
            .mount(&server)
            .await;

        let result = search_at(&reqwest::Client::new(), &server.uri(), "sodium", "1.21.1", &["fabric"], 20, 0).await.unwrap();
        assert_eq!(result.total_hits, 0);
    }
}
