use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    pub is_favorite: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ImageWithGallery {
    #[sqlx(flatten)]
    #[serde(flatten)]
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

    /// List images with pagination, including gallery title, optionally filtered by search query.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
        favorites_only: bool,
        search_query: Option<&str>,
    ) -> Result<Vec<ImageWithGallery>, sqlx::Error> {
        let (sql, has_search): (&str, bool) = match (favorites_only, search_query) {
            (true, Some(_)) => (
                "SELECT i.*, g.title as gallery_title FROM images i LEFT JOIN galleries g ON i.gallery_id = g.id WHERE i.is_favorite = 1 AND (i.original_filename LIKE ?3 OR g.title LIKE ?3) ORDER BY i.created_at DESC LIMIT ?1 OFFSET ?2",
                true,
            ),
            (true, None) => (
                "SELECT i.*, g.title as gallery_title FROM images i LEFT JOIN galleries g ON i.gallery_id = g.id WHERE i.is_favorite = 1 ORDER BY i.created_at DESC LIMIT ?1 OFFSET ?2",
                false,
            ),
            (false, Some(_)) => (
                "SELECT i.*, g.title as gallery_title FROM images i LEFT JOIN galleries g ON i.gallery_id = g.id WHERE (i.original_filename LIKE ?3 OR g.title LIKE ?3) ORDER BY i.created_at DESC LIMIT ?1 OFFSET ?2",
                true,
            ),
            (false, None) => (
                "SELECT i.*, g.title as gallery_title FROM images i LEFT JOIN galleries g ON i.gallery_id = g.id ORDER BY i.created_at DESC LIMIT ?1 OFFSET ?2",
                false,
            ),
        };

        let mut q = sqlx::query_as::<_, ImageWithGallery>(sql)
            .bind(limit)
            .bind(offset);
        if has_search {
            let pattern = format!("%{}%", search_query.unwrap());
            q = q.bind(pattern);
        }
        q.fetch_all(pool).await
    }

    /// Count total images, optionally filtered by search query.
    pub async fn count(pool: &SqlitePool, favorites_only: bool, search_query: Option<&str>) -> Result<i64, sqlx::Error> {
        match (favorites_only, search_query) {
            (true, Some(q)) => {
                let pattern = format!("%{}%", q);
                let row: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM images i LEFT JOIN galleries g ON i.gallery_id = g.id WHERE i.is_favorite = 1 AND (i.original_filename LIKE ?1 OR g.title LIKE ?1)"
                )
                .bind(&pattern)
                .fetch_one(pool)
                .await?;
                Ok(row.0)
            }
            (true, None) => {
                let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM images WHERE is_favorite = 1")
                    .fetch_one(pool)
                    .await?;
                Ok(row.0)
            }
            (false, Some(q)) => {
                let pattern = format!("%{}%", q);
                let row: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM images i LEFT JOIN galleries g ON i.gallery_id = g.id WHERE i.original_filename LIKE ?1 OR g.title LIKE ?1"
                )
                .bind(&pattern)
                .fetch_one(pool)
                .await?;
                Ok(row.0)
            }
            (false, None) => {
                let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM images")
                    .fetch_one(pool)
                    .await?;
                Ok(row.0)
            }
        }
    }

    /// Set favorite status for an image.
    pub async fn set_favorite(pool: &SqlitePool, id: &str, is_favorite: bool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE images SET is_favorite = ? WHERE id = ?")
            .bind(is_favorite)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
