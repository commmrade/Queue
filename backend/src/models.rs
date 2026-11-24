use crate::error::{api_error, parse_uuid, ApiError};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub(crate) struct Claims {
    pub(crate) sub: String,
    pub(crate) jti: String,
    pub(crate) iat: usize,
    pub(crate) exp: usize,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    Admin,
    User,
}

impl Role {
    pub(crate) fn from_database(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(Self::Admin),
            "user" => Some(Self::User),
            _ => None,
        }
    }

    pub(crate) fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
}

#[derive(Clone)]
pub(crate) struct User {
    pub(crate) id: Uuid,
    pub(crate) username: String,
    pub(crate) password_hash: String,
    pub(crate) role: Role,
}

#[derive(FromRow)]
pub(crate) struct AccountRow {
    pub(crate) id: String,
    pub(crate) username: String,
    pub(crate) password_hash: String,
    pub(crate) role: String,
}

impl TryFrom<AccountRow> for User {
    type Error = ApiError;

    fn try_from(row: AccountRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_uuid(&row.id)?,
            username: row.username,
            password_hash: row.password_hash,
            role: Role::from_database(&row.role).ok_or_else(|| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Неизвестная роль пользователя",
                )
            })?,
        })
    }
}

#[derive(Serialize)]
pub(crate) struct PublicUser {
    pub(crate) id: Uuid,
    pub(crate) username: String,
    pub(crate) role: Role,
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            username: user.username.clone(),
            role: user.role,
        }
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct Queue {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) course: String,
    pub(crate) teacher: String,
    pub(crate) subqueues: Vec<Subqueue>,
    pub(crate) created_at: u64,
    pub(crate) activity_at: u64,
    pub(crate) is_open: bool,
    pub(crate) owner_id: Uuid,
}

#[derive(Clone, Serialize)]
pub(crate) struct Entry {
    pub(crate) id: Uuid,
    pub(crate) name: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct Subqueue {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) is_open: bool,
    pub(crate) entries: Vec<Entry>,
}

#[derive(FromRow)]
pub(crate) struct QueueRow {
    pub(crate) id: String,
    pub(crate) owner_id: String,
    pub(crate) title: String,
    pub(crate) course: String,
    pub(crate) teacher: String,
    pub(crate) is_open: bool,
    pub(crate) created_at: u64,
    pub(crate) activity_at: u64,
}

#[derive(FromRow)]
pub(crate) struct SubqueueEntryRow {
    pub(crate) subqueue_id: String,
    pub(crate) subqueue_title: String,
    pub(crate) subqueue_is_open: bool,
    pub(crate) entry_id: Option<String>,
    pub(crate) entry_name: Option<String>,
}

impl TryFrom<QueueRow> for Queue {
    type Error = ApiError;

    fn try_from(row: QueueRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_uuid(&row.id)?,
            owner_id: parse_uuid(&row.owner_id)?,
            title: row.title,
            course: row.course,
            teacher: row.teacher,
            subqueues: Vec::new(),
            created_at: row.created_at,
            activity_at: row.activity_at,
            is_open: row.is_open,
        })
    }
}

#[derive(Serialize)]
pub(crate) struct JoinResponse {
    pub(crate) queue: Queue,
    pub(crate) leave_token: String,
}

#[derive(Deserialize)]
pub(crate) struct Credentials {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Deserialize)]
pub(crate) struct CreateQueue {
    pub(crate) title: String,
    pub(crate) course: String,
    pub(crate) teacher: String,
    #[serde(default)]
    pub(crate) subqueues: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateQueue {
    pub(crate) title: String,
    pub(crate) course: String,
    pub(crate) teacher: String,
    pub(crate) is_open: bool,
}

#[derive(Deserialize)]
pub(crate) struct JoinQueue {
    pub(crate) name: String,
}

#[derive(Deserialize)]
pub(crate) struct CreateSubqueue {
    pub(crate) title: String,
}

#[derive(Deserialize)]
pub(crate) struct UpdateSubqueueTitle {
    pub(crate) title: String,
}

#[derive(Deserialize)]
pub(crate) struct SetSubqueueOpen {
    pub(crate) is_open: bool,
}
