//! SHA1 — единственный алгоритм контрольных сумм, который использует
//! экосистема Mojang/Fabric/Quilt/Modrinth для проверки файлов.

use sha1::{Digest, Sha1};
use std::path::Path;

pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub async fn sha1_hex_file(path: &Path) -> std::io::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    Ok(sha1_hex(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vector() {
        // sha1("") — стандартный тестовый вектор.
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"minecraft"), sha1_hex(b"minecraft"));
        assert_ne!(sha1_hex(b"minecraft"), sha1_hex(b"Minecraft"));
    }
}
