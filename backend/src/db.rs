use crate::{
    error::{api_error, db_error, parse_uuid, ApiError},
    models::{Entry, EntryRow, Queue, QueueRow},
};
use axum::http::StatusCode;
use sqlx::MySqlPool;
use uuid::Uuid;

pub(crate) async fn fetch_queue(db: &MySqlPool, id: Uuid) -> Result<Queue, ApiError> {
    let id = id.to_string();
    let row = sqlx::query_as::<_, QueueRow>(
        "SELECT id, owner_id, title, course, teacher, is_open, created_at FROM queues WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(db)
    .await
    .map_err(db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Очередь не найдена"))?;
    let mut queue = Queue::try_from(row)?;
    let entries = sqlx::query_as::<_, EntryRow>(
        "SELECT id, name FROM queue_entries WHERE queue_id = ? ORDER BY position",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(db_error)?;
    queue.entries = entries
        .into_iter()
        .map(|entry| {
            Ok(Entry {
                id: parse_uuid(&entry.id)?,
                name: entry.name,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(queue)
}
