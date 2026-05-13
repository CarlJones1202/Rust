use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use tracing::error;

use crate::models::gallery::{Gallery, GalleryDetail};
use crate::models::image::Image;
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

    Ok(Json(GalleryDetail { gallery, images }))
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
