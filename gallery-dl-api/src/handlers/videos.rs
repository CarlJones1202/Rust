use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use tracing::error;

use crate::models::video::{Video, VideoProgress};
use crate::pagination::{PaginatedResponse, PaginationMeta, PaginationParams};
use crate::AppState;

/// GET /api/videos — List all videos (paginated).
pub async fn list_videos(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Video>>, (StatusCode, Json<serde_json::Value>)> {
    let total = Video::count(&state.db).await.map_err(|e| {
        error!(error = %e, "Failed to count videos");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Database error" })),
        )
    })?;

    let items = Video::list(&state.db, params.per_page(), params.offset())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list videos");
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

#[derive(Deserialize)]
pub struct ProgressUpdate {
    pub position_seconds: f64,
}

/// POST /api/videos/{id}/progress — Update watch position.
pub async fn save_video_progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update): Json<ProgressUpdate>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    VideoProgress::save(&state.db, &id, update.position_seconds)
        .await
        .map_err(|e| {
            error!(error = %e, video_id = %id, "Failed to save video progress");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?;

    Ok(StatusCode::OK)
}

/// GET /api/videos/{id}/progress — Get last watch position.
pub async fn get_video_progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Option<VideoProgress>>, (StatusCode, Json<serde_json::Value>)> {
    let progress = VideoProgress::get_by_video_id(&state.db, &id)
        .await
        .map_err(|e| {
            error!(error = %e, video_id = %id, "Failed to get video progress");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?;

    Ok(Json(progress))
}
