use axum::http::StatusCode;

pub(crate) type ApiError = (StatusCode, String);

pub(crate) fn api_error(status: StatusCode, message: &str) -> ApiError {
    (status, message.to_owned())
}

pub(crate) fn db_error(error: sqlx::Error) -> ApiError {
    eprintln!("database error: {error}");
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Ошибка базы данных")
}

pub(crate) fn is_duplicate(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db)
        if db.code().as_deref() == Some("1062") || db.message().contains("Duplicate entry"))
}

pub(crate) fn parse_uuid(value: &str) -> Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(value).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "В базе найден некорректный идентификатор",
        )
    })
}

pub(crate) fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
