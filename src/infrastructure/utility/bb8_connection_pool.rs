#![allow(unused)]

use anyhow::{Context, Result};
use bb8::{ManageConnection, PooledConnection};
use diesel_async::{
    AsyncPgConnection,
    pooled_connection::{AsyncDieselConnectionManager, bb8::Pool as DieselPool},
};
use redact::Secret;

pub type PgPool = Bb8ConnectionPool<AsyncDieselConnectionManager<AsyncPgConnection>>;
pub type PooledPgConnection<'a> =
    PooledConnection<'a, AsyncDieselConnectionManager<AsyncPgConnection>>;

#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub database: String,
    pub user_name: Option<String>,
    pub password: Option<Secret<String>>,
    pub min_idle_connection_count: u32,
    pub max_connection_count: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database_url: "localhost:5432".to_string(),
            database: "postgres".to_string(),
            user_name: None,
            password: None,
            min_idle_connection_count: 4,
            max_connection_count: 16,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bb8ConnectionPool<M: ManageConnection> {
    pool: bb8::Pool<M>,
}

impl Bb8ConnectionPool<AsyncDieselConnectionManager<AsyncPgConnection>> {
    pub async fn new(settings: Settings) -> Result<Self> {
        let Settings {
            database_url,
            database,
            user_name,
            password,
            min_idle_connection_count,
            max_connection_count,
        } = settings;
        let pg_url = if !database_url.starts_with("postgres://") {
            let user_name = user_name.context("user name is missing")?;
            let password = password
                .context("password is missing")?
                .expose_secret()
                .to_string();
            format!("postgres://{user_name}:{password}@{database_url}/{database}")
        } else {
            // TODO: math pattern of database url of pg
            database_url
        };
        let manager = AsyncDieselConnectionManager::new(pg_url);
        let pool = DieselPool::builder()
            .min_idle(min_idle_connection_count)
            .max_size(max_connection_count)
            .build(manager)
            .await?;
        Ok(Self { pool })
    }

    /// We can use its transaction after getting connection, like the example:
    ///
    /// Example
    /// ```rust
    /// let mut result = pool
    ///     .get_connection()
    ///     .await?
    ///     .transaction(|mut c| {
    ///         async move {
    ///             let select_result = select(&mut c).await?;
    ///             // do something
    ///             let insert_result = insert(&mut c).await;
    ///             anyhow::Ok(insert_result)
    ///         }
    ///         .scope_boxed()
    ///     })
    ///     .await;
    /// ```
    pub async fn get_connection(&self) -> Result<PooledPgConnection<'_>> {
        let connection = self.pool.get().await?;
        Ok(connection)
    }
}
