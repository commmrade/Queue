mod auth;
mod queues;

use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(queues::health))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::current_user))
        .route("/api/auth/logout", post(auth::logout))
        .route(
            "/api/queues",
            get(queues::list_queues).post(queues::create_queue),
        )
        .route("/api/queues/:id/join", post(queues::join_queue))
        .route("/api/queues/:id/subqueues", post(queues::create_subqueue))
        .route(
            "/api/queues/:id/subqueues/:subqueue_id/open",
            axum::routing::put(queues::set_subqueue_open),
        )
        .route(
            "/api/queues/:id/subqueues/:subqueue_id/join",
            post(queues::join_subqueue),
        )
        .route(
            "/api/queues/:id/entries/:entry_id",
            axum::routing::delete(queues::leave_queue),
        )
        .route(
            "/api/queues/:id/manage/entries/:entry_id",
            axum::routing::delete(queues::remove_entry),
        )
        .route(
            "/api/queues/:id",
            axum::routing::put(queues::update_queue).delete(queues::delete_queue),
        )
        .with_state(state)
}
