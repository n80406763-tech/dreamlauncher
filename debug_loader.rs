use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Простой тест: пробуем скачать Fabric 1.21.1
    let client = reqwest::Client::new();

    println!("Загружаем profile.json от Fabric...");
    let profile_url = "https://meta.fabricmc.net/v2/versions/loader/1.21.1/0.16.9/profile/json";
    let profile_text = client.get(profile_url).send().await?.text().await?;

    let profile: serde_json::Value = serde_json::from_str(&profile_text)?;

    println!("Библиотеки Fabric:");
    if let Some(libraries) = profile["libraries"].as_array() {
        for lib in libraries {
            let name = lib["name"].as_str().unwrap_or("unknown");
            let has_downloads = lib.get("downloads").is_some();
            let url = lib["url"].as_str();

            println!("  - {}", name);
            println!("    downloads: {}", has_downloads);
            println!("    url: {:?}", url);

            // Если нет downloads, проверим доступность по legacy-схеме
            if !has_downloads && url.is_some() {
                let coords = parse_maven_coords(name);
                if let Some((group, artifact, version)) = coords {
                    let base_url = url.unwrap();
                    let path = format!("{}/{}/{}/{}-{}.jar",
                        group.replace('.', "/"),
                        artifact,
                        version,
                        artifact,
                        version
                    );
                    let full_url = format!("{}{}", base_url, path);

                    println!("    Проверяем URL: {}", full_url);
                    match client.head(&full_url).send().await {
                        Ok(resp) => println!("    Статус: {}", resp.status()),
                        Err(e) => println!("    ОШИБКА: {}", e),
                    }
                }
            }
            println!();
        }
    }

    Ok(())
}

fn parse_maven_coords(name: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() >= 3 {
        Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
    } else {
        None
    }
}
