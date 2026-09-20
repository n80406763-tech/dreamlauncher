//! Офлайн-профиль: имя игрока + детерминированный UUID, без сети.
//!
//! Алгоритм — тот же, что использует ванильный сервер/клиент в offline-mode
//! (`UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`):
//! MD5 от строки, с выставленными битами версии (3) и варианта (RFC 4122).
//! Не UUIDv3 в строгом смысле RFC 4122 (там MD5 берётся от `namespace ||
//! name`) — Minecraft хэширует только саму строку, без namespace.

use md5::{Digest, Md5};

pub fn offline_uuid(player_name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{player_name}").as_bytes());
    let mut bytes: [u8; 16] = hasher.finalize().into();

    bytes[6] = (bytes[6] & 0x0f) | 0x30; // version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant RFC 4122

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Имена, допустимые в офлайн-профиле: те же правила, что у Mojang для
/// никнеймов (3-16 символов, латиница/цифры/подчёркивание) — так офлайн- и
/// онлайн-профили ведут себя одинаково предсказуемо.
pub fn is_valid_player_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_reference_uuids() {
        // Значения сверены с независимым вычислением того же алгоритма
        // (Python: MD5("OfflinePlayer:<name>") с теми же битовыми масками)
        // и с широко известным офлайн-UUID "Notch".
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(offline_uuid("Steve"), "5627dd98-e6be-3c21-b8a8-e92344183641");
    }

    #[test]
    fn is_deterministic() {
        assert_eq!(offline_uuid("SomePlayer"), offline_uuid("SomePlayer"));
        assert_ne!(offline_uuid("SomePlayer"), offline_uuid("OtherPlayer"));
    }

    #[test]
    fn validates_player_name_rules() {
        assert!(is_valid_player_name("Steve_123"));
        assert!(!is_valid_player_name("ab")); // слишком короткое
        assert!(!is_valid_player_name(&"a".repeat(17))); // слишком длинное
        assert!(!is_valid_player_name("bad name")); // пробел
        assert!(!is_valid_player_name("héllo")); // не ASCII
    }
}
