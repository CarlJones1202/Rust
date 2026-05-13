use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DownloadRequest {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub image_count: i64,
    pub video_count: i64,
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
    pub async fn create(pool: &SqlitePool, url: &str, title: Option<&str>) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO requests (id, url, title, status) VALUES (?, ?, ?, 'pending')"
        )
        .bind(&id)
        .bind(url)
        .bind(title)
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT r.*, 
                   (SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE g.request_id = r.id) as image_count,
                   (SELECT COUNT(*) FROM videos v WHERE v.request_id = r.id) as video_count
            FROM requests r 
            WHERE r.id = ?
            "#
        )
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Get a request by URL.
    pub async fn get_by_url(pool: &SqlitePool, url: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT r.*, 
                   (SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE g.request_id = r.id) as image_count,
                   (SELECT COUNT(*) FROM videos v WHERE v.request_id = r.id) as video_count
            FROM requests r 
            WHERE r.url = ?
            "#
        )
            .bind(url)
            .fetch_optional(pool)
            .await
    }

    /// List requests with pagination, searching, filtering, and sorting.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
        search: Option<&str>,
        status: Option<&str>,
        sort: Option<&str>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut query_str = r#"
            SELECT r.*, 
                   (SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE g.request_id = r.id) as image_count,
                   (SELECT COUNT(*) FROM videos v WHERE v.request_id = r.id) as video_count
            FROM requests r
        "#.to_string();
        let mut conditions = Vec::new();

        if search.is_some() {
            conditions.push("(url LIKE ? OR title LIKE ?)");
        }
        if status.is_some() {
            conditions.push("status = ?");
        }

        if !conditions.is_empty() {
            query_str.push_str(" WHERE ");
            query_str.push_str(&conditions.join(" AND "));
        }

        let order_by = match sort {
            Some("oldest") => "created_at ASC",
            Some("status_asc") => "status ASC",
            Some("status_desc") => "status DESC",
            Some("title_asc") => "title ASC",
            Some("title_desc") => "title DESC",
            Some("url_asc") => "url ASC",
            Some("url_desc") => "url DESC",
            _ => "created_at DESC",
        };

        query_str.push_str(&format!(" ORDER BY {} LIMIT ? OFFSET ?", order_by));

        let mut query = sqlx::query_as::<_, Self>(&query_str);

        if let Some(s) = search {
            let pattern = format!("%{}%", s);
            query = query.bind(pattern.clone()).bind(pattern);
        }
        if let Some(st) = status {
            query = query.bind(st);
        }

        query.bind(limit).bind(offset).fetch_all(pool).await
    }

    /// Count requests with optional filters.
    pub async fn count(
        pool: &SqlitePool,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let mut query_str = "SELECT COUNT(*) FROM requests".to_string();
        let mut conditions = Vec::new();

        if search.is_some() {
            conditions.push("(url LIKE ? OR title LIKE ?)");
        }
        if status.is_some() {
            conditions.push("status = ?");
        }

        if !conditions.is_empty() {
            query_str.push_str(" WHERE ");
            query_str.push_str(&conditions.join(" AND "));
        }

        let mut query = sqlx::query_as::<_, (i64,)>(&query_str);

        if let Some(s) = search {
            let pattern = format!("%{}%", s);
            query = query.bind(pattern.clone()).bind(pattern);
        }
        if let Some(st) = status {
            query = query.bind(st);
        }

        let row = query.fetch_one(pool).await?;
        Ok(row.0)
    }

    /// List requests that are not yet completed or failed.
    pub async fn list_unfinished(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT r.*, 
                   (SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE g.request_id = r.id) as image_count,
                   (SELECT COUNT(*) FROM videos v WHERE v.request_id = r.id) as video_count
            FROM requests r 
            WHERE r.status IN ('pending', 'processing') 
            ORDER BY r.created_at ASC
            "#
        )
        .fetch_all(pool)
        .await
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
