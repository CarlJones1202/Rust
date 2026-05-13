use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DownloadRequest {
    pub id: String,
    pub url: String,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Response that includes nested galleries and videos.
#[derive(Debug, Serialize)]
pub struct DownloadRequestDetail {
    #[serde(flatten)]
    pub request: DownloadRequest,
    pub galleries: Vec<super::gallery::Gallery>,
    pub videos: Vec<super::video::Video>,
}

impl DownloadRequest {
    /// Create a new download request.
    pub async fn create(pool: &SqlitePool, url: &str) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO requests (id, url, status) VALUES (?, ?, 'pending')"
        )
        .bind(&id)
        .bind(url)
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Get a request by ID.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM requests WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// List requests with pagination.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM requests ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    /// Count total requests.
    pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM requests")
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }

    /// Update request status.
    pub async fn update_status(
        pool: &SqlitePool,
        id: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE requests SET status = ?, error_message = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(status)
        .bind(error_message)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }
}
