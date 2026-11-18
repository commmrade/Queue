use crate::{
    auth::{authenticated_user, optional_user_id, visitor_id_cookie, visitor_id_from_headers},
    db::fetch_queue,
    error::{api_error, db_error, is_duplicate, now_seconds, parse_uuid, ApiError},
    models::{CreateQueue, Entry, JoinQueue, JoinResponse, Queue, QueueListRow, UpdateQueue},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
struct EntrySecretRow {
    leave_token_hash: String,
}

pub(super) async fn list_queues(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let rows = sqlx::query_as::<_, QueueListRow>(
        "SELECT q.id, q.owner_id, q.title, q.course, q.teacher, q.is_open, q.created_at, e.id AS entry_id, e.name AS entry_name FROM queues q LEFT JOIN queue_entries e ON e.queue_id = q.id ORDER BY q.created_at DESC, q.id, e.position",
    )
    .fetch_all(&state.db)
    .await
    .map_err(db_error)?;

    let mut queues = Vec::with_capacity(rows.len());
    let mut positions = std::collections::HashMap::new();
    for row in rows {
        let id = parse_uuid(&row.id)?;
        let position = if let Some(position) = positions.get(&row.id) {
            *position
        } else {
            let position = queues.len();
            positions.insert(row.id.clone(), position);
            queues.push(Queue {
                id,
                owner_id: parse_uuid(&row.owner_id)?,
                title: row.title,
                course: row.course,
                teacher: row.teacher,
                entries: Vec::new(),
                created_at: row.created_at,
                is_open: row.is_open,
            });
            position
        };
        if let (Some(entry_id), Some(name)) = (row.entry_id, row.entry_name) {
            queues[position].entries.push(Entry {
                id: parse_uuid(&entry_id)?,
                name,
            });
        }
    }
    let mut response = Json(queues).into_response();
    if visitor_id_from_headers(&headers).is_none() {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, visitor_id_cookie(Uuid::new_v4()));
    }
    Ok(response)
}

pub(super) async fn create_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateQueue>,
) -> Result<Json<Queue>, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    if !user.role.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Создавать очереди могут только администраторы",
        ));
    }
    let title = input.title.trim();
    if title.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Укажите название показа",
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO queues (id, owner_id, title, course, teacher, is_open, created_at) VALUES (?, ?, ?, ?, ?, TRUE, ?)")
        .bind(id.to_string())
        .bind(user.id.to_string())
        .bind(title)
        .bind(input.course.trim())
        .bind(input.teacher.trim())
        .bind(now_seconds())
        .execute(&state.db)
        .await
        .map_err(db_error)?;
    Ok(Json(fetch_queue(&state.db, id).await?))
}

pub(super) async fn update_queue(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UpdateQueue>,
) -> Result<Json<Queue>, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    if !user.role.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Управлять очередями могут только администраторы",
        ));
    }
    let title = input.title.trim();
    if title.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Укажите название показа",
        ));
    }
    let result = sqlx::query(
        "UPDATE queues SET title = ?, course = ?, teacher = ?, is_open = ? WHERE id = ?",
    )
    .bind(title)
    .bind(input.course.trim())
    .bind(input.teacher.trim())
    .bind(input.is_open)
    .bind(id.to_string())
    .execute(&state.db)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM queues WHERE id = ?)")
            .bind(id.to_string())
            .fetch_one(&state.db)
            .await
            .map_err(db_error)?;
        if !exists {
            return Err(api_error(StatusCode::NOT_FOUND, "Очередь не найдена"));
        }
    }
    Ok(Json(fetch_queue(&state.db, id).await?))
}

pub(super) async fn join_queue(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<JoinQueue>,
) -> Result<impl IntoResponse, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "Укажите фамилию"));
    }
    let user_id = optional_user_id(&headers, &state).await;
    let existing_visitor_id = visitor_id_from_headers(&headers);
    let visitor_id = existing_visitor_id.unwrap_or_else(Uuid::new_v4);
    let should_set_visitor_cookie = existing_visitor_id.is_none();
    let queue_id = id.to_string();
    let is_open: Option<bool> = sqlx::query_scalar("SELECT is_open FROM queues WHERE id = ?")
        .bind(&queue_id)
        .fetch_optional(&state.db)
        .await
        .map_err(db_error)?;
    match is_open {
        None => return Err(api_error(StatusCode::NOT_FOUND, "Очередь не найдена")),
        Some(false) => {
            return Err(api_error(
                StatusCode::CONFLICT,
                "Запись в эту очередь закрыта",
            ));
        }
        Some(true) => {}
    }
    let entry_id = Uuid::new_v4();
    let leave_token = Uuid::new_v4().to_string();
    let leave_token_hash = hex::encode(Sha256::digest(leave_token.as_bytes()));
    sqlx::query("INSERT INTO queue_entries (id, queue_id, user_id, visitor_id, name, leave_token_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(entry_id.to_string())
        .bind(&queue_id)
        .bind(user_id.map(|value| value.to_string()))
        .bind(visitor_id.to_string())
        .bind(name)
        .bind(leave_token_hash)
        .bind(now_seconds())
        .execute(&state.db)
        .await
        .map_err(|error| {
            if is_duplicate(&error) {
                api_error(StatusCode::CONFLICT, "Вы уже записаны в эту очередь")
            } else {
                db_error(error)
            }
        })?;
    let mut response = Json(JoinResponse {
        queue: fetch_queue(&state.db, id).await?,
        leave_token,
    })
    .into_response();
    if should_set_visitor_cookie {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, visitor_id_cookie(visitor_id));
    }
    Ok(response)
}

pub(super) async fn leave_queue(
    Path((queue_id, entry_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Queue>, ApiError> {
    let token = headers
        .get("x-queue-leave-token")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            api_error(
                StatusCode::FORBIDDEN,
                "Для выхода используйте устройство, с которого записывались",
            )
        })?;
    let entry = sqlx::query_as::<_, EntrySecretRow>(
        "SELECT leave_token_hash FROM queue_entries WHERE id = ? AND queue_id = ?",
    )
    .bind(entry_id.to_string())
    .bind(queue_id.to_string())
    .fetch_optional(&state.db)
    .await
    .map_err(db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Запись не найдена"))?;
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    if entry.leave_token_hash != token_hash {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Удалить можно только собственную запись",
        ));
    }
    sqlx::query("DELETE FROM queue_entries WHERE id = ?")
        .bind(entry_id.to_string())
        .execute(&state.db)
        .await
        .map_err(db_error)?;
    Ok(Json(fetch_queue(&state.db, queue_id).await?))
}

pub(super) async fn remove_entry(
    Path((queue_id, entry_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Queue>, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    if !user.role.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Управлять очередями могут только администраторы",
        ));
    }
    let result = sqlx::query("DELETE FROM queue_entries WHERE id = ? AND queue_id = ?")
        .bind(entry_id.to_string())
        .bind(queue_id.to_string())
        .execute(&state.db)
        .await
        .map_err(db_error)?;
    if result.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM queues WHERE id = ?)")
            .bind(queue_id.to_string())
            .fetch_one(&state.db)
            .await
            .map_err(db_error)?;
        return Err(api_error(
            StatusCode::NOT_FOUND,
            if exists {
                "Запись не найдена"
            } else {
                "Очередь не найдена"
            },
        ));
    }
    Ok(Json(fetch_queue(&state.db, queue_id).await?))
}

pub(super) async fn delete_queue(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    if !user.role.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Управлять очередями могут только администраторы",
        ));
    }
    let result = sqlx::query("DELETE FROM queues WHERE id = ?")
        .bind(id.to_string())
        .execute(&state.db)
        .await
        .map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err(api_error(StatusCode::NOT_FOUND, "Очередь не найдена"));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn health(State(state): State<AppState>) -> StatusCode {
    if sqlx::query("SELECT 1").execute(&state.db).await.is_ok() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
