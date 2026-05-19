use sqlx::SqlitePool;
use tracing::info;

use crate::models::person::{find_person_id_by_name, link_gallery_person};
use crate::services::title_guesser;

/// Auto-link a gallery to a person if the URL's person name matches an existing person.
/// Uses the same URL-based person name guessing as the gallery title guesser,
/// then looks up the name against persons (including aliases).
pub async fn auto_link_gallery(
    pool: &SqlitePool,
    url: &str,
    gallery_id: &str,
) -> Result<bool, sqlx::Error> {
    let Some(person_name) = title_guesser::guess_person_name_from_url(url) else {
        return Ok(false);
    };

    let Some(person_id) = find_person_id_by_name(pool, &person_name).await? else {
        return Ok(false);
    };

    link_gallery_person(pool, gallery_id, &person_id).await?;
    info!(
        person_name = %person_name,
        person_id = %person_id,
        gallery_id = %gallery_id,
        "Auto-linked gallery to person"
    );
    Ok(true)
}

/// When a new person is created, scan all completed download requests and
/// retro-actively link any galleries whose URL person name matches.
pub async fn retroactively_link_person(
    pool: &SqlitePool,
    person_id: &str,
    person_name: &str,
) -> Result<u64, sqlx::Error> {
    let requests: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, url FROM requests WHERE status = 'completed'"
    )
    .fetch_all(pool)
    .await?;

    let name_lower = person_name.to_lowercase();
    let mut linked_count = 0u64;

    for (req_id, url) in &requests {
        let Some(candidate_name) = title_guesser::guess_person_name_from_url(url) else {
            continue;
        };

        if candidate_name.to_lowercase() != name_lower {
            continue;
        }

        let galleries: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM galleries WHERE request_id = ?"
        )
        .bind(req_id)
        .fetch_all(pool)
        .await?;

        for (gallery_id,) in &galleries {
            link_gallery_person(pool, gallery_id, person_id).await?;
            linked_count += 1;
        }
    }

    if linked_count > 0 {
        info!(
            person_name = %person_name,
            person_id = %person_id,
            linked = linked_count,
            "Retro-actively linked person to galleries"
        );
    }

    Ok(linked_count)
}
