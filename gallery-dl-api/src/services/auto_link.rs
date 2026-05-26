use percent_encoding::percent_decode_str;
use sqlx::SqlitePool;
use std::collections::HashSet;
use tracing::info;

use crate::models::person::{link_gallery_person, PersonAlias};

/// Normalize a URL string for matching: percent-decode, lowercase, replace separators with spaces.
pub fn normalize_for_matching(text: &str) -> String {
    let decoded = percent_decode_str(text)
        .decode_utf8_lossy()
        .into_owned();
    decoded
        .to_lowercase()
        .replace('-', " ")
        .replace('/', " ")
        .replace('.', " ")
        .replace('_', " ")
        .replace('?', " ")
        .replace('&', " ")
        .replace('#', " ")
        .replace('=', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Auto-link a gallery to any people whose name or alias appears anywhere in the URL.
/// Searches all person names and aliases against the normalized URL text,
/// matching longest names first to prefer "Bob Jones" over "Bob".
pub async fn auto_link_gallery(
    pool: &SqlitePool,
    url: &str,
    gallery_id: &str,
) -> Result<u64, sqlx::Error> {
    let people: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM persons")
        .fetch_all(pool)
        .await?;

    let aliases: Vec<(String, String)> = sqlx::query_as(
        "SELECT alias, person_id FROM person_aliases",
    )
    .fetch_all(pool)
    .await?;

    let mut candidates: Vec<(String, String)> = people
        .into_iter()
        .chain(aliases.into_iter().map(|(alias, pid)| (pid, alias)))
        .filter(|(_, text)| text.len() > 1)
        .collect();

    candidates.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    let normalized_url = normalize_for_matching(url);
    let mut linked_count = 0u64;
    let mut linked_ids = HashSet::new();

    for (person_id, search_text) in &candidates {
        if linked_ids.contains(person_id) {
            continue;
        }
        if normalized_url.contains(&search_text.to_lowercase()) {
            link_gallery_person(pool, gallery_id, person_id).await?;
            linked_ids.insert(person_id.clone());
            linked_count += 1;
            info!(
                person_name = %search_text,
                person_id = %person_id,
                gallery_id = %gallery_id,
                "Auto-linked gallery to person"
            );
        }
    }

    Ok(linked_count)
}

/// When a new person is created, scan all completed download requests and
/// retro-actively link any galleries whose URL contains the person's name or aliases.
pub async fn retroactively_link_person(
    pool: &SqlitePool,
    person_id: &str,
    person_name: &str,
) -> Result<u64, sqlx::Error> {
    let aliases = PersonAlias::get_for_person(pool, person_id)
        .await
        .unwrap_or_default();

    let mut search_terms: Vec<String> = vec![person_name.to_string()];
    search_terms.extend(aliases);

    let requests: Vec<(String, String)> = sqlx::query_as(
        "SELECT r.id, r.url FROM requests r WHERE r.status = 'completed'",
    )
    .fetch_all(pool)
    .await?;

    let mut linked_count = 0u64;

    for (req_id, url) in &requests {
        let normalized = normalize_for_matching(url);

        if !search_terms
            .iter()
            .any(|term| normalized.contains(&term.to_lowercase()))
        {
            continue;
        }

        let galleries: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM galleries WHERE request_id = ?")
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
