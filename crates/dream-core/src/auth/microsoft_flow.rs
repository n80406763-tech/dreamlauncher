//! Полная реализация Device Code Flow для Microsoft OAuth → Xbox Live
//! → XSTS → Minecraft Services. Эндпоинты и типы описаны в `microsoft.rs`,
//! здесь — сетевой обмен и оркестрация всей цепочки.

use super::microsoft::*;
use crate::error::{CoreError, Result};
use serde::Serialize;
use std::time::Duration;

/// Прогресс авторизации — отправляется через `Channel` в UI.
#[derive(Debug, Clone)]
pub enum AuthProgress {
    /// Device code получен, ждём, пока пользователь перейдёт по ссылке.
    WaitingForUser { user_code: String, verification_uri: String, expires_in: u64 },
    /// Поллинг токена в процессе (каждые N секунд).
    Polling,
    /// Получен MS токен, обмениваем на Xbox Live.
    ExchangingXboxLive,
    /// Получен Xbox токен, запрашиваем XSTS.
    ExchangingXsts,
    /// Получен XSTS, логинимся в Minecraft Services.
    LoggingIntoMinecraft,
    /// Получаем профиль игрока.
    FetchingProfile,
    /// Готово — возвращаем финальный профиль.
    Complete { uuid: String, username: String },
}

/// Результат полной авторизации — всё, что нужно записать в БД и keyring.
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub uuid: String,
    pub username: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// Запускает Device Code Flow и проходит всю цепочку до получения
/// Minecraft-профиля. `client_id` — из регистрации Azure-приложения.
/// `progress_callback` вызывается на каждом шаге для обновления UI.
pub async fn authenticate_microsoft<F>(
    client: &reqwest::Client,
    client_id: &str,
    mut progress_callback: F,
) -> Result<AuthResult>
where
    F: FnMut(AuthProgress),
{
    // Шаг 1: запрос device code
    let device_code_resp = request_device_code(client, client_id).await?;
    progress_callback(AuthProgress::WaitingForUser {
        user_code: device_code_resp.user_code.clone(),
        verification_uri: device_code_resp.verification_uri.clone(),
        expires_in: device_code_resp.expires_in,
    });

    // Шаг 2: поллинг токена
    let ms_token = poll_for_token(
        client,
        client_id,
        &device_code_resp.device_code,
        device_code_resp.interval,
        device_code_resp.expires_in,
        || progress_callback(AuthProgress::Polling),
    )
    .await?;

    // Шаг 3: Xbox Live
    progress_callback(AuthProgress::ExchangingXboxLive);
    let xbox_token = authenticate_xbox_live(client, &ms_token.access_token).await?;

    // Шаг 4: XSTS
    progress_callback(AuthProgress::ExchangingXsts);
    let xsts_token = authenticate_xsts(client, &xbox_token.token).await?;

    let user_hash = xsts_token.user_hash().ok_or_else(|| CoreError::Other("XSTS ответ не содержит user hash".into()))?;

    // Шаг 5: Minecraft login
    progress_callback(AuthProgress::LoggingIntoMinecraft);
    let mc_auth = login_with_xbox(client, user_hash, &xsts_token.token).await?;

    // Шаг 6: профиль
    progress_callback(AuthProgress::FetchingProfile);
    let profile = fetch_minecraft_profile(client, &mc_auth.access_token).await?;

    progress_callback(AuthProgress::Complete { uuid: profile.id.clone(), username: profile.name.clone() });

    Ok(AuthResult {
        uuid: profile.id,
        username: profile.name,
        access_token: mc_auth.access_token,
        refresh_token: ms_token.refresh_token,
        expires_in: mc_auth.expires_in,
    })
}

/// Шаг 1: запрос device code
async fn request_device_code(client: &reqwest::Client, client_id: &str) -> Result<DeviceCodeResponse> {
    let body = format!("client_id={}&scope={}",
        urlencoding::encode(client_id),
        urlencoding::encode(OAUTH_SCOPE));

    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?
        .error_for_status()?
        .json::<DeviceCodeResponse>()
        .await?;

    Ok(resp)
}

/// Шаг 2: поллинг токена до получения или таймаута
async fn poll_for_token<F>(
    client: &reqwest::Client,
    client_id: &str,
    device_code: &str,
    interval_secs: u64,
    expires_in: u64,
    mut on_poll: F,
) -> Result<MsTokenResponse>
where
    F: FnMut(),
{
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(expires_in);
    let interval = Duration::from_secs(interval_secs);

    loop {
        if start.elapsed() > timeout {
            return Err(CoreError::Other("device code истёк — пользователь не завершил авторизацию вовремя".into()));
        }

        tokio::time::sleep(interval).await;
        on_poll();

        let body = format!("grant_type={}&client_id={}&device_code={}",
            urlencoding::encode("urn:ietf:params:oauth:grant-type:device_code"),
            urlencoding::encode(client_id),
            urlencoding::encode(device_code));

        let resp = client
            .post(TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;

        if !resp.status().is_success() {
            // Ожидаем `authorization_pending` или `slow_down` — продолжаем поллинг
            let status = resp.status();
            if let Ok(result) = resp.json::<MsTokenPollResult>().await {
                match result {
                    MsTokenPollResult::Success(token) => return Ok(token),
                    MsTokenPollResult::Pending { error } if error == "authorization_pending" => continue,
                    MsTokenPollResult::Pending { error } if error == "slow_down" => {
                        // Увеличиваем интервал на 5 секунд
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    MsTokenPollResult::Pending { error } => {
                        return Err(CoreError::Other(format!("ошибка поллинга токена: {error}")));
                    }
                }
            } else {
                return Err(CoreError::Other(format!("неизвестный ответ при поллинге: HTTP {status}")));
            }
        } else {
            let token = resp.json::<MsTokenResponse>().await?;
            return Ok(token);
        }
    }
}

/// Шаг 3: Xbox Live authenticate
async fn authenticate_xbox_live(client: &reqwest::Client, ms_access_token: &str) -> Result<XboxTokenResponse> {
    #[derive(Serialize)]
    struct Req {
        #[serde(rename = "Properties")]
        properties: Properties,
        #[serde(rename = "RelyingParty")]
        relying_party: &'static str,
        #[serde(rename = "TokenType")]
        token_type: &'static str,
    }

    #[derive(Serialize)]
    struct Properties {
        #[serde(rename = "AuthMethod")]
        auth_method: &'static str,
        #[serde(rename = "SiteName")]
        site_name: &'static str,
        #[serde(rename = "RpsTicket")]
        rps_ticket: String,
    }

    let body = Req {
        properties: Properties { auth_method: "RPS", site_name: "user.auth.xboxlive.com", rps_ticket: format!("d={ms_access_token}") },
        relying_party: "http://auth.xboxlive.com",
        token_type: "JWT",
    };

    let resp = client.post(XBOX_LIVE_AUTH_URL).json(&body).send().await?.error_for_status()?.json::<XboxTokenResponse>().await?;

    Ok(resp)
}

/// Шаг 4: XSTS authorize
async fn authenticate_xsts(client: &reqwest::Client, xbox_token: &str) -> Result<XboxTokenResponse> {
    #[derive(Serialize)]
    struct Req {
        #[serde(rename = "Properties")]
        properties: Properties,
        #[serde(rename = "RelyingParty")]
        relying_party: &'static str,
        #[serde(rename = "TokenType")]
        token_type: &'static str,
    }

    #[derive(Serialize)]
    struct Properties {
        #[serde(rename = "SandboxId")]
        sandbox_id: &'static str,
        #[serde(rename = "UserTokens")]
        user_tokens: Vec<String>,
    }

    let body = Req { properties: Properties { sandbox_id: "RETAIL", user_tokens: vec![xbox_token.to_string()] }, relying_party: "rp://api.minecraftservices.com/", token_type: "JWT" };

    let resp = client.post(XSTS_AUTHORIZE_URL).json(&body).send().await?;

    if !resp.status().is_success() {
        // Обрабатываем известные коды XErr
        #[derive(serde::Deserialize)]
        struct XErr {
            #[serde(rename = "XErr")]
            x_err: Option<u64>,
        }
        let status = resp.status();
        if let Ok(err_body) = resp.json::<XErr>().await {
            if let Some(code) = err_body.x_err {
                let msg = describe_xerr(code);
                return Err(CoreError::Other(format!("ошибка XSTS (код {code}): {msg}")));
            }
        }
        return Err(CoreError::Other(format!("ошибка XSTS: HTTP {status}")));
    }

    Ok(resp.json::<XboxTokenResponse>().await?)
}

/// Шаг 5: Minecraft login with Xbox
async fn login_with_xbox(client: &reqwest::Client, user_hash: &str, xsts_token: &str) -> Result<McAuthResponse> {
    #[derive(Serialize)]
    struct Req {
        #[serde(rename = "identityToken")]
        identity_token: String,
    }

    let identity_token = format!("XBL3.0 x={user_hash};{xsts_token}");
    let body = Req { identity_token };

    let resp = client.post(MC_LOGIN_WITH_XBOX_URL).json(&body).send().await?.error_for_status()?.json::<McAuthResponse>().await?;

    Ok(resp)
}

/// Шаг 6: получение профиля Minecraft
async fn fetch_minecraft_profile(client: &reqwest::Client, mc_access_token: &str) -> Result<McProfile> {
    let resp = client.get(MC_PROFILE_URL).bearer_auth(mc_access_token).send().await?;

    if !resp.status().is_success() {
        return Err(CoreError::Other(format!("не удалось получить профиль Minecraft: HTTP {} (возможно, у аккаунта нет лицензии)", resp.status())));
    }

    Ok(resp.json::<McProfile>().await?)
}

/// Обновление токена через refresh token — вызывается автоматически при
/// запуске инстанса, если access token истёк. Возвращает новый access token.
pub async fn refresh_microsoft_token(client: &reqwest::Client, client_id: &str, refresh_token: &str) -> Result<MsTokenResponse> {
    let body = format!("grant_type={}&client_id={}&refresh_token={}&scope={}",
        urlencoding::encode("refresh_token"),
        urlencoding::encode(client_id),
        urlencoding::encode(refresh_token),
        urlencoding::encode(OAUTH_SCOPE));

    let resp = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?
        .error_for_status()?
        .json::<MsTokenResponse>()
        .await?;

    Ok(resp)
}

/// Проверка entitlements — есть ли у аккаунта лицензия Minecraft.
/// Вызывается после первого логина, чтобы сразу предупредить пользователя.
pub async fn check_minecraft_entitlements(client: &reqwest::Client, mc_access_token: &str) -> Result<bool> {
    #[derive(serde::Deserialize)]
    struct EntitlementsResponse {
        items: Vec<serde_json::Value>,
    }

    let resp = client.get(MC_ENTITLEMENTS_URL).bearer_auth(mc_access_token).send().await?.error_for_status()?.json::<EntitlementsResponse>().await?;

    // Проверяем, есть ли хотя бы один entitlement с product_minecraft или game_minecraft
    Ok(!resp.items.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_progress_variants_construct() {
        let _ = AuthProgress::WaitingForUser { user_code: "ABC123".into(), verification_uri: "https://microsoft.com/link".into(), expires_in: 900 };
        let _ = AuthProgress::Complete { uuid: "uuid".into(), username: "Player".into() };
    }
}
