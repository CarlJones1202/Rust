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
    pub title: Option<String>,
    pub file_size_bytes: i64,
    pub duration_seconds: Option<f64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub progress_seconds: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct VideoProgress {
    pub video_id: String,
    pub position_seconds: f64,
    pub updated_at: String,
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
        duration_seconds: Option<f64>,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO videos (id, request_id, hash, extension, original_filename, file_size_bytes, duration_seconds, width, height) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(request_id)
        .bind(hash)
        .bind(extension)
        .bind(original_filename)
        .bind(file_size_bytes)
        .bind(duration_seconds)
        .bind(width)
        .bind(height)
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Get a video by ID.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT v.*, vp.position_seconds as progress_seconds 
             FROM videos v 
             LEFT JOIN video_progress vp ON v.id = vp.video_id 
             WHERE v.id = ?"
        )
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
            "SELECT v.*, vp.position_seconds as progress_seconds 
             FROM videos v 
             LEFT JOIN video_progress vp ON v.id = vp.video_id 
             WHERE v.request_id = ? 
             ORDER BY v.created_at ASC"
        )
        .bind(request_id)
        .fetch_all(pool)
        .await
    }

    /// List videos with pagination, optionally filtered by search query.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
        search_query: Option<&str>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let (sql, has_search) = if search_query.is_some() {
            (
                "SELECT v.*, vp.position_seconds as progress_seconds 
                 FROM videos v 
                 LEFT JOIN video_progress vp ON v.id = vp.video_id 
                 WHERE (v.title LIKE ?3 OR v.original_filename LIKE ?3) 
                 ORDER BY v.created_at DESC 
                 LIMIT ?1 OFFSET ?2",
                true,
            )
        } else {
            (
                "SELECT v.*, vp.position_seconds as progress_seconds 
                 FROM videos v 
                 LEFT JOIN video_progress vp ON v.id = vp.video_id 
                 ORDER BY v.created_at DESC 
                 LIMIT ?1 OFFSET ?2",
                false,
            )
        };

        let mut q = sqlx::query_as::<_, Self>(sql)
            .bind(limit)
            .bind(offset);
        if has_search {
            let pattern = format!("%{}%", search_query.unwrap());
            q = q.bind(pattern);
        }
        q.fetch_all(pool).await
    }

    pub async fn count(pool: &SqlitePool, search_query: Option<&str>) -> Result<i64, sqlx::Error> {
        let (sql, has_search) = if search_query.is_some() {
            ("SELECT COUNT(*) FROM videos WHERE title LIKE ?1 OR original_filename LIKE ?1", true)
        } else {
            ("SELECT COUNT(*) FROM videos", false)
        };

        let mut q = sqlx::query_as::<_, (i64,)>(sql);
        if has_search {
            let pattern = format!("%{}%", search_query.unwrap());
            q = q.bind(pattern);
        }
        let row: (i64,) = q.fetch_one(pool).await?;
        Ok(row.0)
    }

    /// Update video title.
    pub async fn update_title(
        pool: &SqlitePool,
        id: &str,
        title: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET title = ? WHERE id = ?")
            .bind(title)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

impl VideoProgress {
    /// Save or update video progress.
    pub async fn save(
        pool: &SqlitePool,
        video_id: &str,
        position_seconds: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO video_progress (video_id, position_seconds, updated_at) 
             VALUES (?, ?, datetime('now'))
             ON CONFLICT(video_id) DO UPDATE SET 
                position_seconds = excluded.position_seconds,
                updated_at = datetime('now')"
        )
        .bind(video_id)
        .bind(position_seconds)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get video progress by video ID.
    pub async fn get_by_video_id(
        pool: &SqlitePool,
        video_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM video_progress WHERE video_id = ?")
            .bind(video_id)
            .fetch_optional(pool)
            .await
    }
}
