use sqlx::{mysql::MySqlPoolOptions, MySqlPool};
use std::{env, sync::Arc};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: MySqlPool,
    pub(crate) jwt_secret: Arc<Vec<u8>>,
}

impl AppState {
    pub(crate) async fn connect() -> Self {
        let jwt_secret = env::var("QUEUE_JWT_SECRET")
            .expect("QUEUE_JWT_SECRET must be configured")
            .into_bytes();
        if jwt_secret.len() < 32 {
            panic!("QUEUE_JWT_SECRET must contain at least 32 bytes");
        }

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
        let db = MySqlPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .expect("could not connect to MariaDB");
        sqlx::migrate!()
            .run(&db)
            .await
            .expect("database migrations failed");

        Self {
            db,
            jwt_secret: Arc::new(jwt_secret),
        }
    }
}
