use crate::{
    auth::{
        authenticated_user, clear_jwt_cookie, create_token, decode_token, jwt_cookie,
        normalize_username, token_from_headers,
    },
    error::{api_error, db_error, is_duplicate, now_seconds, ApiError},
    models::{AccountRow, Credentials, PublicUser, Role, User},
    state::AppState,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use rand_core::OsRng;

pub(super) async fn register(
    State(state): State<AppState>,
    Json(input): Json<Credentials>,
) -> Result<impl IntoResponse, ApiError> {
    let username = normalize_username(&input.username).ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "Имя пользователя: 3–32 символа, латинские буквы, цифры и _",
        )
    })?;
    if !(8..=128).contains(&input.password.len()) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Пароль должен содержать от 8 до 128 символов",
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(input.password.as_bytes(), &salt)
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Не удалось создать аккаунт",
            )
        })?
        .to_string();
    let user = User {
        id: uuid::Uuid::new_v4(),
        username: username.clone(),
        password_hash: password_hash.clone(),
        role: Role::User,
    };
    let token = create_token(user.id, &state)?;
    sqlx::query("INSERT INTO accounts (id, username, password_hash, role, created_at) VALUES (?, ?, ?, 'user', ?)")
        .bind(user.id.to_string())
        .bind(&username)
        .bind(password_hash)
        .bind(now_seconds())
        .execute(&state.db)
        .await
        .map_err(|error| {
            if is_duplicate(&error) {
                api_error(StatusCode::CONFLICT, "Это имя пользователя уже занято")
            } else {
                db_error(error)
            }
        })?;

    let mut response = Json(PublicUser::from(&user)).into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, jwt_cookie(&token));
    Ok(response)
}

pub(super) async fn login(
    State(state): State<AppState>,
    Json(input): Json<Credentials>,
) -> Result<impl IntoResponse, ApiError> {
    let username = normalize_username(&input.username).ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "Имя пользователя: 3–32 символа, латинские буквы, цифры и _",
        )
    })?;
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, username, password_hash, role FROM accounts WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await
    .map_err(db_error)?
    .ok_or_else(|| {
        api_error(
            StatusCode::UNAUTHORIZED,
            "Неверное имя пользователя или пароль",
        )
    })?;
    let user = User::try_from(row)?;
    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Не удалось проверить пароль",
        )
    })?;
    if Argon2::default()
        .verify_password(input.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "Неверное имя пользователя или пароль",
        ));
    }
    let token = create_token(user.id, &state)?;
    let mut response = Json(PublicUser::from(&user)).into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, jwt_cookie(&token));
    Ok(response)
}

pub(super) async fn current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PublicUser>, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    Ok(Json(PublicUser::from(&user)))
}

pub(super) async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(token) = token_from_headers(&headers) {
        if let Ok(claims) = decode_token(&token, &state) {
            sqlx::query("DELETE FROM revoked_tokens WHERE expires_at < ?")
                .bind(now_seconds())
                .execute(&state.db)
                .await
                .map_err(db_error)?;
            sqlx::query(
                "INSERT INTO revoked_tokens (jti, expires_at) VALUES (?, ?) ON DUPLICATE KEY UPDATE expires_at = VALUES(expires_at)",
            )
            .bind(claims.jti)
            .bind(claims.exp as u64)
            .execute(&state.db)
            .await
            .map_err(db_error)?;
        }
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, clear_jwt_cookie());
    Ok(response)
}
