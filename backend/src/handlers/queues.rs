use crate::{
    auth::{authenticated_user, optional_user_id, visitor_id_cookie, visitor_id_from_headers},
    db::fetch_queue,
    error::{api_error, db_error, is_duplicate, now_seconds, parse_uuid, ApiError},
    models::{
        CreateQueue, CreateSubqueue, JoinQueue, JoinResponse, Queue, SetSubqueueOpen,
        UpdateQueue, UpdateSubqueueTitle,
    },
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
    let ids = sqlx::query_scalar::<_, String>(
        "SELECT q.id FROM queues q ORDER BY CASE WHEN q.is_open = FALSE OR NOT EXISTS (SELECT 1 FROM queue_subqueues s WHERE s.queue_id = q.id AND s.is_open = TRUE) THEN 1 ELSE 0 END ASC, q.activity_at DESC, q.created_at DESC, q.id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(db_error)?;
    let mut queues = Vec::with_capacity(ids.len());
    for id in ids {
        queues.push(fetch_queue(&state.db, parse_uuid(&id)?).await?);
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
    let mut subqueue_titles: Vec<String> = input
        .subqueues
        .into_iter()
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
        .collect();
    if subqueue_titles.is_empty() {
        subqueue_titles.push("Общая очередь".to_owned());
    }
    if subqueue_titles
        .iter()
        .any(|title| title.chars().count() > 120)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Название подочереди не должно превышать 120 символов",
        ));
    }
    let mut transaction = state.db.begin().await.map_err(db_error)?;
    let created_at = now_seconds();
    sqlx::query("INSERT INTO queues (id, owner_id, title, course, teacher, is_open, created_at, activity_at) VALUES (?, ?, ?, ?, ?, TRUE, ?, ?)")
        .bind(id.to_string())
        .bind(user.id.to_string())
        .bind(title)
        .bind(input.course.trim())
        .bind(input.teacher.trim())
        .bind(created_at)
        .bind(created_at)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for (position, subqueue_title) in subqueue_titles.iter().enumerate() {
        sqlx::query("INSERT INTO queue_subqueues (id, queue_id, title, position, is_open) VALUES (?, ?, ?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(id.to_string())
            .bind(subqueue_title)
            .bind(position as u64)
            .bind(position == 0)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                if is_duplicate(&error) {
                    api_error(StatusCode::CONFLICT, "Названия подочередей должны отличаться")
                } else {
                    db_error(error)
                }
            })?;
    }
    transaction.commit().await.map_err(db_error)?;
    Ok(Json(fetch_queue(&state.db, id).await?))
}

pub(super) async fn create_subqueue(
    Path(queue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSubqueue>,
) -> Result<Json<Queue>, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    if !user.role.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Управлять очередями могут только администраторы",
        ));
    }
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Укажите название подочереди (до 120 символов)",
        ));
    }
    let queue_id_string = queue_id.to_string();
    let max_position: Option<u64> =
        sqlx::query_scalar("SELECT MAX(position) FROM queue_subqueues WHERE queue_id = ?")
            .bind(&queue_id_string)
            .fetch_one(&state.db)
            .await
            .map_err(db_error)?;
    let position = max_position.map_or(0, |position| position.saturating_add(1));
    sqlx::query("INSERT INTO queue_subqueues (id, queue_id, title, position, is_open) VALUES (?, ?, ?, ?, FALSE)")
        .bind(Uuid::new_v4().to_string())
        .bind(queue_id_string)
        .bind(title)
        .bind(position)
        .execute(&state.db)
        .await
        .map_err(|error| {
            if is_duplicate(&error) {
                api_error(StatusCode::CONFLICT, "Подочередь с таким названием уже есть")
            } else {
                db_error(error)
            }
        })?;
    Ok(Json(fetch_queue(&state.db, queue_id).await?))
}

pub(super) async fn rename_subqueue(
    Path((queue_id, subqueue_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UpdateSubqueueTitle>,
) -> Result<Json<Queue>, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    if !user.role.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Управлять очередями могут только администраторы",
        ));
    }
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Укажите название подочереди (до 120 символов)",
        ));
    }
    let result = sqlx::query("UPDATE queue_subqueues SET title = ? WHERE id = ? AND queue_id = ?")
        .bind(title)
        .bind(subqueue_id.to_string())
        .bind(queue_id.to_string())
        .execute(&state.db)
        .await
        .map_err(|error| {
            if is_duplicate(&error) {
                api_error(StatusCode::CONFLICT, "Подочередь с таким названием уже есть")
            } else {
                db_error(error)
            }
        })?;
    if result.rows_affected() == 0 {
        return Err(api_error(StatusCode::NOT_FOUND, "Подочередь не найдена"));
    }
    Ok(Json(fetch_queue(&state.db, queue_id).await?))
}

pub(super) async fn set_subqueue_open(
    Path((queue_id, subqueue_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SetSubqueueOpen>,
) -> Result<Json<Queue>, ApiError> {
    let user = authenticated_user(&headers, &state).await?;
    if !user.role.is_admin() {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Управлять очередями могут только администраторы",
        ));
    }
    let current_is_open: Option<bool> =
        sqlx::query_scalar("SELECT is_open FROM queue_subqueues WHERE id = ? AND queue_id = ?")
            .bind(subqueue_id.to_string())
            .bind(queue_id.to_string())
            .fetch_optional(&state.db)
            .await
            .map_err(db_error)?;
    let current_is_open =
        current_is_open.ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Подочередь не найдена"))?;
    sqlx::query("UPDATE queue_subqueues SET is_open = ? WHERE id = ? AND queue_id = ?")
        .bind(input.is_open)
        .bind(subqueue_id.to_string())
        .bind(queue_id.to_string())
        .execute(&state.db)
        .await
        .map_err(db_error)?;
    if current_is_open != input.is_open {
        sqlx::query("UPDATE queues SET activity_at = ? WHERE id = ?")
            .bind(now_seconds())
            .bind(queue_id.to_string())
            .execute(&state.db)
            .await
            .map_err(db_error)?;
    }
    Ok(Json(fetch_queue(&state.db, queue_id).await?))
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
    let result = sqlx::query("UPDATE queues SET title = ?, course = ?, teacher = ?, activity_at = IF(is_open <> ?, ?, activity_at), is_open = ? WHERE id = ?")
    .bind(title)
    .bind(input.course.trim())
    .bind(input.teacher.trim())
    .bind(input.is_open)
    .bind(now_seconds())
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
    join_subqueue_impl(id, None, state, headers, input).await
}

pub(super) async fn join_subqueue(
    Path((id, subqueue_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<JoinQueue>,
) -> Result<impl IntoResponse, ApiError> {
    join_subqueue_impl(id, Some(subqueue_id), state, headers, input).await
}

async fn join_subqueue_impl(
    id: Uuid,
    subqueue_id: Option<Uuid>,
    state: AppState,
    headers: HeaderMap,
    input: JoinQueue,
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
    let queue_is_open: Option<bool> = sqlx::query_scalar("SELECT is_open FROM queues WHERE id = ?")
        .bind(&queue_id)
        .fetch_optional(&state.db)
        .await
        .map_err(db_error)?;
    match queue_is_open {
        None => return Err(api_error(StatusCode::NOT_FOUND, "Очередь не найдена")),
        Some(false) => {
            return Err(api_error(
                StatusCode::CONFLICT,
                "Запись в эту очередь закрыта",
            ));
        }
        Some(true) => {}
    }
    let target_subqueue_id = if let Some(subqueue_id) = subqueue_id {
        subqueue_id
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM queue_subqueues WHERE queue_id = ? AND is_open = TRUE ORDER BY position LIMIT 1",
        )
        .bind(&queue_id)
        .fetch_optional(&state.db)
        .await
        .map_err(db_error)?
        .map(|id| parse_uuid(&id))
        .transpose()?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Подочередь не найдена"))?
    };
    let subqueue_open: Option<bool> =
        sqlx::query_scalar("SELECT is_open FROM queue_subqueues WHERE id = ? AND queue_id = ?")
            .bind(target_subqueue_id.to_string())
            .bind(&queue_id)
            .fetch_optional(&state.db)
            .await
            .map_err(db_error)?;
    match subqueue_open {
        None => return Err(api_error(StatusCode::NOT_FOUND, "Подочередь не найдена")),
        Some(false) => {
            return Err(api_error(
                StatusCode::CONFLICT,
                "Запись в эту подочередь закрыта",
            ))
        }
        Some(true) => {}
    }
    let entry_id = Uuid::new_v4();
    let leave_token = Uuid::new_v4().to_string();
    let leave_token_hash = hex::encode(Sha256::digest(leave_token.as_bytes()));
    sqlx::query("INSERT INTO queue_entries (id, queue_id, subqueue_id, user_id, visitor_id, name, leave_token_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(entry_id.to_string())
        .bind(&queue_id)
        .bind(target_subqueue_id.to_string())
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
