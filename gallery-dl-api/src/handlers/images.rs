use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use tracing::error;

use crate::models::image::{Image, ImageWithGallery};
use crate::pagination::{PaginatedResponse, PaginationMeta, PaginationParams};
use crate::AppState;

/// GET /api/images — List all images (paginated).
pub async fn list_images(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ImageWithGallery>>, (StatusCode, Json<serde_json::Value>)> {
    let favorites_only = params.favorites.as_deref() == Some("true");

    let total = Image::count(&state.db, favorites_only).await.map_err(|e| {
        error!(error = %e, "Failed to count images");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Database error" })),
        )
    })?;

    let items = Image::list(&state.db, params.per_page(), params.offset(), favorites_only)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list images");
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

/// PATCH /api/images/:id/favorite — Toggle favorite status on an image.
pub async fn toggle_favorite(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<Image>, (StatusCode, Json<serde_json::Value>)> {
    let is_favorite = body.get("is_favorite").and_then(|v| v.as_bool()).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "is_favorite must be a boolean" })),
        )
    })?;

    let _image = Image::get_by_id(&state.db, &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get image");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Image not found" })),
            )
        })?;

    Image::set_favorite(&state.db, &id, is_favorite)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update favorite status");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?;

    let updated = Image::get_by_id(&state.db, &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get updated image");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Image not found after update" })),
            )
        })?;

    Ok(Json(updated))
}
