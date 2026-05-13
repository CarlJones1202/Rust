use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Video {
    pub id: String,
    pub request_id: String,
    pub hash: String,
    pub extension: String,
    pub original_filename: Option<String>,
    pub file_size_bytes: i64,
    pub created_at: String,
}

impl Video {
    /// Create a new video record.
    pub async fn create(
        pool: &SqlitePool,
        request_id: &str,
        hash: &str,
        extension: &str,
        original_filename: Option<&str>,
        file_size_bytes: i64,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO videos (id, request_id, hash, extension, original_filename, file_size_bytes) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(request_id)
        .bind(hash)
        .bind(extension)
        .bind(original_filename)
        .bind(file_size_bytes)
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Get a video by ID.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM videos WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Get videos by request ID.
    pub async fn get_by_request_id(
        pool: &SqlitePool,
        request_id: &str,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM videos WHERE request_id = ? ORDER BY created_at ASC"
        )
        .bind(request_id)
        .fetch_all(pool)
        .await
    }

    /// List videos with pagination.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM videos ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    /// Count total videos.
    pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM videos")
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}
