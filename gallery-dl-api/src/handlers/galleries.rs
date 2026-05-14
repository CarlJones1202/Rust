use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use tracing::error;

use crate::models::gallery::{Gallery, GalleryDetail};
use crate::models::image::Image;
use crate::models::person::get_persons_for_gallery;
use crate::pagination::{PaginatedResponse, PaginationMeta, PaginationParams};
use crate::AppState;

/// GET /api/galleries — List all galleries (paginated).
pub async fn list_galleries(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Gallery>>, (StatusCode, Json<serde_json::Value>)> {
    let total = Gallery::count(&state.db).await.map_err(|e| {
        error!(error = %e, "Failed to count galleries");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Database error" })),
        )
    })?;

    let items = Gallery::list(&state.db, params.per_page(), params.offset())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list galleries");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: items,
        pagination: PaginationMeta::new(params.page(), params.per_page(), total),
    }))
}

/// GET /api/galleries/:id — Get a gallery with its images.
pub async fn get_gallery(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GalleryDetail>, (StatusCode, Json<serde_json::Value>)> {
    let gallery = Gallery::get_by_id(&state.db, &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get gallery");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Gallery not found" })),
            )
        })?;

    let images = Image::get_by_gallery_id(&state.db, &id)
        .await
        .unwrap_or_default();

    let persons = get_persons_for_gallery(&state.db, &id)
        .await
        .unwrap_or_default();

    Ok(Json(GalleryDetail { gallery, images, persons }))
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateGalleryBody {
    pub title: String,
}

/// PATCH /api/galleries/:id — Update gallery metadata (e.g. rename).
pub async fn update_gallery(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateGalleryBody>,
) -> Result<Json<Gallery>, (StatusCode, Json<serde_json::Value>)> {
    let gallery = Gallery::get_by_id(&state.db, &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get gallery");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Gallery not found" })),
            )
        })?;

    Gallery::update_title(&state.db, &gallery.id, &body.title)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update gallery title");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to update gallery title" })),
            )
        })?;

    let updated = Gallery::get_by_id(&state.db, &id)
        .await
        .unwrap() // Should exist
        .unwrap();

    Ok(Json(updated))
}

/// POST /api/galleries/retroactive-update — Guess and update titles for all unnamed galleries and requests.
pub async fn retroactive_update_titles(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Update Requests first
    let requests: Vec<(String, String)> = sqlx::query_as("SELECT id, url FROM requests WHERE title IS NULL OR title = ''")
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch unnamed requests");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "Database error" })))
        })?;

    let mut request_updated = 0;
    for (id, url) in requests {
        if let Some(guessed) = crate::services::title_guesser::guess_title(&state.db, &url).await {
            let _ = sqlx::query("UPDATE requests SET title = ? WHERE id = ?")
                .bind(&guessed)
                .bind(&id)
                .execute(&state.db)
                .await;
            request_updated += 1;
        }
    }

    // Update Galleries
    let galleries: Vec<(String, String)> = sqlx::query_as(
        "SELECT g.id, r.url FROM galleries g JOIN requests r ON g.request_id = r.id WHERE g.title IS NULL OR g.title = ''"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch unnamed galleries");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "Database error" })))
    })?;

    let mut gallery_updated = 0;
    for (id, url) in galleries {
        if let Some(guessed) = crate::services::title_guesser::guess_title(&state.db, &url).await {
            let _ = Gallery::update_title(&state.db, &id, &guessed).await;
            gallery_updated += 1;
        }
    }

    Ok(Json(serde_json::json!({ 
        "requests_updated": request_updated,
        "galleries_updated": gallery_updated 
    })))
}
