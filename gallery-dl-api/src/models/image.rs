use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Image {
    pub id: String,
    pub gallery_id: String,
    pub hash: String,
    pub extension: String,
    pub original_filename: Option<String>,
    pub file_size_bytes: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub top_colors: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ImageWithGallery {
    #[sqlx(flatten)]
    pub image: Image,
    pub gallery_title: Option<String>,
}

impl Image {
    /// Create a new image record.
    pub async fn create(
        pool: &SqlitePool,
        gallery_id: &str,
        hash: &str,
        extension: &str,
        original_filename: Option<&str>,
        file_size_bytes: i64,
        width: Option<i32>,
        height: Option<i32>,
        top_colors: Option<String>,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO images (id, gallery_id, hash, extension, original_filename, file_size_bytes, width, height, top_colors) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(gallery_id)
        .bind(hash)
        .bind(extension)
        .bind(original_filename)
        .bind(file_size_bytes)
        .bind(width)
        .bind(height)
        .bind(top_colors)
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Get an image by ID.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM images WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Get images by gallery ID.
    pub async fn get_by_gallery_id(
        pool: &SqlitePool,
        gallery_id: &str,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM images WHERE gallery_id = ? ORDER BY created_at ASC"
        )
        .bind(gallery_id)
        .fetch_all(pool)
        .await
    }

    /// List images with pagination, including gallery title.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ImageWithGallery>, sqlx::Error> {
        sqlx::query_as::<_, ImageWithGallery>(
            "SELECT i.*, g.title as gallery_title FROM images i LEFT JOIN galleries g ON i.gallery_id = g.id ORDER BY i.created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }

    /// Count total images.
    pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM images")
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}
