use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Gallery {
    pub id: String,
    pub request_id: String,
    pub title: Option<String>,
    pub created_at: String,
}

/// Gallery with nested images for detail responses.
#[derive(Debug, Serialize)]
pub struct GalleryDetail {
    #[serde(flatten)]
    pub gallery: Gallery,
    pub images: Vec<super::image::Image>,
}

impl Gallery {
    /// Create a new gallery linked to a request.
    pub async fn create(
        pool: &SqlitePool,
        request_id: &str,
        title: Option<&str>,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO galleries (id, request_id, title) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(request_id)
            .bind(title)
            .execute(pool)
            .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Get a gallery by ID.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM galleries WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Get galleries by request ID.
    pub async fn get_by_request_id(
        pool: &SqlitePool,
        request_id: &str,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM galleries WHERE request_id = ?")
            .bind(request_id)
            .fetch_all(pool)
            .await
    }

    /// List galleries with pagination.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM galleries ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    /// Count total galleries.
    pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM galleries")
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}
