//! Авторизация через Microsoft: Device Code → Xbox Live → XSTS →
//! Minecraft Services. Эндпоинты и структуры запросов/ответов даны по
//! документации (RFC 8628 для device-flow, публичные описания
//! Xbox Live/XSTS/Minecraft Services API) и НЕ протестированы вживую —
//! для этого нужна регистрация Azure-приложения (public client, personal
//! Microsoft accounts) и заявка в Mojang на доступ к Minecraft API (см.
//! план, раздел 2). Это первая часть M3; сам сетевой обмен и `keyring`-
//! хранение токенов делаются поверх этих типов.

use serde::{Deserialize, Serialize};

pub const AZURE_TENANT: &str = "consumers";
pub const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
pub const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
pub const XBOX_LIVE_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
pub const XSTS_AUTHORIZE_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
pub const MC_LOGIN_WITH_XBOX_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
pub const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
pub const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
pub const OAUTH_SCOPE: &str = "XboxLive.signin offline_access";

/// Известные коды ошибок XSTS (`XErr`) — стоит показывать пользователю
/// человеко-понятным текстом вместо сырого числа.
pub const XERR_NO_XBOX_ACCOUNT: u64 = 2148916233;
pub const XERR_CHILD_ACCOUNT: u64 = 2148916238;
pub const XERR_COUNTRY_UNAVAILABLE: u64 = 2148916235;

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MsTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

/// Ответ на poll до завершения логина — либо токен, либо
/// `error: "authorization_pending" | "slow_down" | "expired_token" | "access_denied"`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MsTokenPollResult {
    Success(MsTokenResponse),
    Pending { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayClaimsXui {
    pub uhs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayClaims {
    pub xui: Vec<DisplayClaimsXui>,
}

/// Общая форма ответа и Xbox Live `/authenticate`, и XSTS `/authorize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XboxTokenResponse {
    #[serde(rename = "Token")]
    pub token: String,
    #[serde(rename = "DisplayClaims")]
    pub display_claims: DisplayClaims,
}

impl XboxTokenResponse {
    pub fn user_hash(&self) -> Option<&str> {
        self.display_claims.xui.first().map(|x| x.uhs.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct McAuthResponse {
    pub username: String,
    pub access_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McProfile {
    pub id: String,
    pub name: String,
}

/// Значение `Authorization` для XSTS: `XBL3.0 x=<userhash>;<xsts_token>`.
pub fn xbox_authorization_header(user_hash: &str, xsts_token: &str) -> String {
    format!("XBL3.0 x={user_hash};{xsts_token}")
}

/// Человекочитаемое объяснение известных кодов `XErr` — используется, когда
/// XSTS отвечает 401 с телом `{"XErr": ..., "Message": ..., "Redirect": ...}`.
pub fn describe_xerr(code: u64) -> &'static str {
    match code {
        XERR_NO_XBOX_ACCOUNT => "На этом аккаунте Microsoft нет учётной записи Xbox — её нужно создать на xbox.com.",
        XERR_CHILD_ACCOUNT => "Аккаунт принадлежит ребёнку — нужно добавить его в семейную группу с разрешением на Xbox.",
        XERR_COUNTRY_UNAVAILABLE => "Xbox Live недоступен в стране/регионе этого аккаунта.",
        _ => "Неизвестная ошибка Xbox Live/XSTS.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_xbox_authorization_header() {
        assert_eq!(xbox_authorization_header("abc123", "tok.en"), "XBL3.0 x=abc123;tok.en");
    }

    #[test]
    fn extracts_user_hash_from_display_claims() {
        let response = XboxTokenResponse { token: "t".into(), display_claims: DisplayClaims { xui: vec![DisplayClaimsXui { uhs: "hash123".into() }] } };
        assert_eq!(response.user_hash(), Some("hash123"));
    }

    #[test]
    fn known_xerr_codes_have_human_readable_text() {
        assert!(describe_xerr(XERR_NO_XBOX_ACCOUNT).contains("Xbox"));
        assert_eq!(describe_xerr(999999999), "Неизвестная ошибка Xbox Live/XSTS.");
    }

    #[test]
    fn parses_device_code_response_shape() {
        let json = r#"{
            "device_code": "abc",
            "user_code": "XYZ123",
            "verification_uri": "https://microsoft.com/link",
            "expires_in": 900,
            "interval": 5,
            "message": "To sign in, use a web browser..."
        }"#;
        let parsed: DeviceCodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.user_code, "XYZ123");
    }
}
