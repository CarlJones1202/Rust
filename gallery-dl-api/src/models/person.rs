use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub disambiguation: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,
    pub country: Option<String>,
    pub height: Option<i32>,
    pub hair_color: Option<String>,
    pub eye_color: Option<String>,
    pub measurements: Option<String>,
    pub breast_type: Option<String>,
    pub career_start_year: Option<i32>,
    pub career_end_year: Option<i32>,
    pub bio: Option<String>,
    pub extra_data: Option<String>, // JSON string for arbitrary key/value pairs
    pub stashdb_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PersonAlias {
    pub id: String,
    pub person_id: String,
    pub alias: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PersonImage {
    pub id: String,
    pub person_id: String,
    pub hash: String,
    pub extension: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub is_primary: i32,
    pub source_url: Option<String>,
    pub created_at: String,
}

/// Summary for embedding in gallery responses.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PersonSummary {
    pub id: String,
    pub name: String,
    pub image_hash: Option<String>,
    pub image_extension: Option<String>,
}

/// Full detail response including aliases, images, and gallery count.
#[derive(Debug, Serialize)]
pub struct PersonDetail {
    #[serde(flatten)]
    pub person: Person,
    pub aliases: Vec<String>,
    pub images: Vec<PersonImage>,
    pub gallery_count: i64,
}

/// Input for creating/updating a person.
#[derive(Debug, Deserialize)]
pub struct PersonInput {
    pub name: Option<String>,
    pub disambiguation: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,
    pub country: Option<String>,
    pub height: Option<i32>,
    pub hair_color: Option<String>,
    pub eye_color: Option<String>,
    pub measurements: Option<String>,
    pub breast_type: Option<String>,
    pub career_start_year: Option<i32>,
    pub career_end_year: Option<i32>,
    pub bio: Option<String>,
    pub extra_data: Option<serde_json::Value>,
    pub aliases: Option<Vec<String>>,
}

impl Person {
    /// Create a new person.
    pub async fn create(pool: &SqlitePool, name: &str) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO persons (id, name) VALUES (?, ?)")
            .bind(&id)
            .bind(name)
            .execute(pool)
            .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Get a person by ID.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM persons WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Update person metadata.
    pub async fn update(pool: &SqlitePool, id: &str, input: &PersonInput) -> Result<(), sqlx::Error> {
        // Build dynamic update query
        let mut sets = Vec::new();
        let mut values: Vec<Option<String>> = Vec::new();

        if let Some(ref name) = input.name {
            sets.push("name = ?");
            values.push(Some(name.clone()));
        }

        // We handle each optional field - if the input provides it, we update it
        if input.disambiguation.is_some() {
            sets.push("disambiguation = ?");
            values.push(input.disambiguation.clone());
        }
        if input.gender.is_some() {
            sets.push("gender = ?");
            values.push(input.gender.clone());
        }
        if input.ethnicity.is_some() {
            sets.push("ethnicity = ?");
            values.push(input.ethnicity.clone());
        }
        if input.country.is_some() {
            sets.push("country = ?");
            values.push(input.country.clone());
        }
        if input.height.is_some() {
            sets.push("height = ?");
            values.push(input.height.map(|v| v.to_string()));
        }
        if input.hair_color.is_some() {
            sets.push("hair_color = ?");
            values.push(input.hair_color.clone());
        }
        if input.eye_color.is_some() {
            sets.push("eye_color = ?");
            values.push(input.eye_color.clone());
        }
        if input.measurements.is_some() {
            sets.push("measurements = ?");
            values.push(input.measurements.clone());
        }
        if input.breast_type.is_some() {
            sets.push("breast_type = ?");
            values.push(input.breast_type.clone());
        }
        if input.career_start_year.is_some() {
            sets.push("career_start_year = ?");
            values.push(input.career_start_year.map(|v| v.to_string()));
        }
        if input.career_end_year.is_some() {
            sets.push("career_end_year = ?");
            values.push(input.career_end_year.map(|v| v.to_string()));
        }
        if input.bio.is_some() {
            sets.push("bio = ?");
            values.push(input.bio.clone());
        }
        if input.extra_data.is_some() {
            sets.push("extra_data = ?");
            values.push(input.extra_data.as_ref().map(|v| v.to_string()));
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = datetime('now')");
        let sql = format!("UPDATE persons SET {} WHERE id = ?", sets.join(", "));

        let mut query = sqlx::query(&sql);
        for v in &values {
            query = query.bind(v.as_deref());
        }
        query = query.bind(id);
        query.execute(pool).await?;

        Ok(())
    }

    /// Delete a person.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM persons WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List persons with pagination and optional search (returns summaries).
    pub async fn list_summaries(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
        search: Option<&str>,
    ) -> Result<Vec<PersonSummary>, sqlx::Error> {
        if let Some(q) = search {
            let pattern = format!("%{}%", q);
            sqlx::query_as::<_, PersonSummary>(
                "SELECT p.id, p.name, p.disambiguation, pi.hash as image_hash, pi.extension as image_extension
                 FROM persons p
                 LEFT JOIN person_aliases pa ON pa.person_id = p.id
                 LEFT JOIN person_images pi ON pi.person_id = p.id AND pi.is_primary = 1
                 WHERE p.name LIKE ? OR pa.alias LIKE ?
                 GROUP BY p.id
                 ORDER BY p.name ASC LIMIT ? OFFSET ?"
            )
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, PersonSummary>(
                "SELECT p.id, p.name, p.disambiguation, pi.hash as image_hash, pi.extension as image_extension
                 FROM persons p
                 LEFT JOIN person_images pi ON pi.person_id = p.id AND pi.is_primary = 1
                 GROUP BY p.id
                 ORDER BY p.name ASC LIMIT ? OFFSET ?"
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    }

    /// Count total persons (with optional search).
    pub async fn count(pool: &SqlitePool, search: Option<&str>) -> Result<i64, sqlx::Error> {
        if let Some(q) = search {
            let pattern = format!("%{}%", q);
            let row: (i64,) = sqlx::query_as(
                "SELECT COUNT(DISTINCT p.id) FROM persons p
                 LEFT JOIN person_aliases pa ON pa.person_id = p.id
                 WHERE p.name LIKE ? OR pa.alias LIKE ?"
            )
            .bind(&pattern)
            .bind(&pattern)
            .fetch_one(pool)
            .await?;
            Ok(row.0)
        } else {
            let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM persons")
                .fetch_one(pool)
                .await?;
            Ok(row.0)
        }
    }

    /// Set stashdb_id on a person.
    pub async fn set_stashdb_id(pool: &SqlitePool, id: &str, stashdb_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE persons SET stashdb_id = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(stashdb_id)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

// === Aliases ===

impl PersonAlias {
    /// Replace all aliases for a person.
    pub async fn set_aliases(pool: &SqlitePool, person_id: &str, aliases: &[String]) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM person_aliases WHERE person_id = ?")
            .bind(person_id)
            .execute(pool)
            .await?;

        for alias in aliases {
            let id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO person_aliases (id, person_id, alias) VALUES (?, ?, ?)")
                .bind(&id)
                .bind(person_id)
                .bind(alias)
                .execute(pool)
                .await?;
        }
        Ok(())
    }

    /// Get all aliases for a person.
    pub async fn get_for_person(pool: &SqlitePool, person_id: &str) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT alias FROM person_aliases WHERE person_id = ? ORDER BY alias"
        )
        .bind(person_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}

// === Person Images ===

impl PersonImage {
    /// Add an image to a person.
    pub async fn create(
        pool: &SqlitePool,
        person_id: &str,
        hash: &str,
        extension: &str,
        width: Option<i32>,
        height: Option<i32>,
        is_primary: bool,
        source_url: Option<&str>,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4().to_string();

        // If this is primary, unset others
        if is_primary {
            sqlx::query("UPDATE person_images SET is_primary = 0 WHERE person_id = ?")
                .bind(person_id)
                .execute(pool)
                .await?;
        }

        sqlx::query(
            "INSERT INTO person_images (id, person_id, hash, extension, width, height, is_primary, source_url)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(person_id)
        .bind(hash)
        .bind(extension)
        .bind(width)
        .bind(height)
        .bind(if is_primary { 1 } else { 0 })
        .bind(source_url)
        .execute(pool)
        .await?;

        sqlx::query_as::<_, Self>("SELECT * FROM person_images WHERE id = ?")
            .bind(&id)
            .fetch_one(pool)
            .await
    }

    /// Get all images for a person.
    pub async fn get_for_person(pool: &SqlitePool, person_id: &str) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM person_images WHERE person_id = ? ORDER BY is_primary DESC, created_at ASC"
        )
        .bind(person_id)
        .fetch_all(pool)
        .await
    }

    /// Delete an image.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM person_images WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Set an image as primary (unsets all others for same person).
    pub async fn set_primary(pool: &SqlitePool, person_id: &str, image_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE person_images SET is_primary = 0 WHERE person_id = ?")
            .bind(person_id)
            .execute(pool)
            .await?;
        sqlx::query("UPDATE person_images SET is_primary = 1 WHERE id = ? AND person_id = ?")
            .bind(image_id)
            .bind(person_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

// === Gallery <-> Person linking ===

/// Link a gallery to a person.
pub async fn link_gallery_person(pool: &SqlitePool, gallery_id: &str, person_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO gallery_persons (gallery_id, person_id) VALUES (?, ?)")
        .bind(gallery_id)
        .bind(person_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Unlink a gallery from a person.
pub async fn unlink_gallery_person(pool: &SqlitePool, gallery_id: &str, person_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM gallery_persons WHERE gallery_id = ? AND person_id = ?")
        .bind(gallery_id)
        .bind(person_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Get all persons linked to a gallery (summary view).
pub async fn get_persons_for_gallery(pool: &SqlitePool, gallery_id: &str) -> Result<Vec<PersonSummary>, sqlx::Error> {
    sqlx::query_as::<_, PersonSummary>(
        "SELECT p.id, p.name, pi.hash as image_hash, pi.extension as image_extension
         FROM persons p
         INNER JOIN gallery_persons gp ON gp.person_id = p.id
         LEFT JOIN person_images pi ON pi.person_id = p.id AND pi.is_primary = 1
         WHERE gp.gallery_id = ?
         ORDER BY p.name"
    )
    .bind(gallery_id)
    .fetch_all(pool)
    .await
}

/// Get gallery count for a person.
pub async fn gallery_count_for_person(pool: &SqlitePool, person_id: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM gallery_persons WHERE person_id = ?"
    )
    .bind(person_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Find a person ID by exact case-insensitive name or alias match.
pub async fn find_person_id_by_name(pool: &SqlitePool, name: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT p.id FROM persons p WHERE LOWER(p.name) = LOWER(?)
         UNION
         SELECT pa.person_id FROM person_aliases pa WHERE LOWER(pa.alias) = LOWER(?)
         LIMIT 1"
    )
    .bind(name)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}
