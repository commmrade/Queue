use sqlx::{mysql::MySqlPoolOptions, MySqlPool};
use std::{env, sync::Arc, time::Duration};

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
        let db = connect_to_database(&database_url).await;
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

async fn connect_to_database(database_url: &str) -> MySqlPool {
    const MAX_ATTEMPTS: u32 = 30;

    for attempt in 1..=MAX_ATTEMPTS {
        match MySqlPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
        {
            Ok(db) => return db,
            Err(error) if matches!(&error, sqlx::Error::Io(_)) && attempt < MAX_ATTEMPTS => {
                eprintln!("MariaDB is not ready (attempt {attempt}/{MAX_ATTEMPTS}); retrying in 1 second: {error}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(error) => {
                panic!("could not connect to MariaDB after {attempt} attempt(s): {error}");
            }
        }
    }

    unreachable!("database connection loop always returns or panics")
}
