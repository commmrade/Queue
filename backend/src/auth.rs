use crate::{
    error::{api_error, db_error, now_seconds, parse_uuid, ApiError},
    models::{AccountRow, Claims, User},
    state::AppState,
};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::env;
use uuid::Uuid;

pub(crate) fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        if let Some(token) = value.strip_prefix("Bearer ") {
            return Some(token.to_owned());
        }
    }
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == "queue_access_token").then(|| value.to_owned())
        })
}

pub(crate) fn visitor_id_from_headers(headers: &HeaderMap) -> Option<Uuid> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == "queue_visitor_id")
                .then(|| Uuid::parse_str(value).ok())
                .flatten()
        })
}

pub(crate) fn visitor_id_cookie(visitor_id: Uuid) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "queue_visitor_id={visitor_id}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000{}",
        secure_cookie_suffix()
    ))
    .expect("generated guest cookie is valid")
}

pub(crate) fn decode_token(token: &str, state: &AppState) -> Result<Claims, ApiError> {
    let validation = Validation::new(Algorithm::HS256);
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_slice()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| api_error(StatusCode::UNAUTHORIZED, "JWT недействителен или истёк"))
}

pub(crate) fn create_token(user_id: Uuid, state: &AppState) -> Result<String, ApiError> {
    let now = now_seconds() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        jti: Uuid::new_v4().to_string(),
        iat: now,
        exp: now + 7 * 24 * 60 * 60,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_slice()),
    )
    .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Не удалось создать JWT"))
}

pub(crate) async fn authenticated_user(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<User, ApiError> {
    let token = token_from_headers(headers)
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Войдите в аккаунт"))?;
    let claims = decode_token(&token, state)?;
    let user_id = parse_uuid(&claims.sub)
        .map_err(|_| api_error(StatusCode::UNAUTHORIZED, "JWT содержит неверный аккаунт"))?;
    let revoked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = ? AND expires_at >= ?)",
    )
    .bind(&claims.jti)
    .bind(now_seconds())
    .fetch_one(&state.db)
    .await
    .map_err(db_error)?;
    if revoked {
        return Err(api_error(StatusCode::UNAUTHORIZED, "JWT отозван"));
    }
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, username, password_hash, role FROM accounts WHERE id = ?",
    )
    .bind(user_id.to_string())
    .fetch_optional(&state.db)
    .await
    .map_err(db_error)?
    .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Пользователь не найден"))?;
    User::try_from(row)
}

pub(crate) async fn optional_user_id(headers: &HeaderMap, state: &AppState) -> Option<Uuid> {
    let token = token_from_headers(headers)?;
    let claims = decode_token(&token, state).ok()?;
    let id = Uuid::parse_str(&claims.sub).ok()?;
    let revoked_or_missing: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = ? AND expires_at >= ?) OR NOT EXISTS(SELECT 1 FROM accounts WHERE id = ?)",
    )
    .bind(&claims.jti)
    .bind(now_seconds())
    .bind(id.to_string())
    .fetch_one(&state.db)
    .await
    .ok()?;
    (!revoked_or_missing).then_some(id)
}

fn secure_cookie_suffix() -> &'static str {
    if env::var("QUEUE_COOKIE_SECURE").as_deref() == Ok("true") {
        "; Secure"
    } else {
        ""
    }
}

pub(crate) fn jwt_cookie(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "queue_access_token={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800{}",
        secure_cookie_suffix()
    ))
    .expect("generated JWT cookie is valid")
}

pub(crate) fn clear_jwt_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "queue_access_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        secure_cookie_suffix()
    ))
    .expect("expired JWT cookie is valid")
}

pub(crate) fn normalize_username(raw: &str) -> Option<String> {
    let username = raw.trim().to_lowercase();
    ((3..=32).contains(&username.len())
        && username.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        }))
    .then_some(username)
}
