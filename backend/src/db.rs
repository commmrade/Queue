use crate::{
    error::{api_error, db_error, parse_uuid, ApiError},
    models::{Entry, Queue, QueueRow, Subqueue},
};
use axum::http::StatusCode;
use sqlx::MySqlPool;
use uuid::Uuid;

pub(crate) async fn fetch_queue(db: &MySqlPool, id: Uuid) -> Result<Queue, ApiError> {
    let id = id.to_string();
    let row = sqlx::query_as::<_, QueueRow>(
        "SELECT id, owner_id, title, course, teacher, is_open, created_at, activity_at FROM queues WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(db)
    .await
    .map_err(db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Очередь не найдена"))?;
    let mut queue = Queue::try_from(row)?;
    let rows = sqlx::query_as::<_, crate::models::SubqueueEntryRow>(
        "SELECT s.id AS subqueue_id, s.title AS subqueue_title, s.is_open AS subqueue_is_open, e.id AS entry_id, e.name AS entry_name FROM queue_subqueues s LEFT JOIN queue_entries e ON e.subqueue_id = s.id WHERE s.queue_id = ? ORDER BY s.position, e.position",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(db_error)?;
    for row in rows {
        let subqueue_index = match queue
            .subqueues
            .iter()
            .position(|subqueue| subqueue.id.to_string() == row.subqueue_id)
        {
            Some(index) => index,
            None => {
                let index = queue.subqueues.len();
                queue.subqueues.push(Subqueue {
                    id: parse_uuid(&row.subqueue_id)?,
                    title: row.subqueue_title,
                    is_open: row.subqueue_is_open,
                    entries: Vec::new(),
                });
                index
            }
        };
        if let (Some(id), Some(name)) = (row.entry_id, row.entry_name) {
            queue.subqueues[subqueue_index].entries.push(Entry {
                id: parse_uuid(&id)?,
                name,
            });
        }
    }
    Ok(queue)
}
