use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use tracing::error;

use crate::models::gallery::Gallery;
use crate::models::request::{DownloadRequest, DownloadRequestDetail};
use crate::models::video::Video;
use crate::pagination::{PaginatedResponse, PaginationMeta, PaginationParams};
use crate::queue::worker::DownloadJob;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateRequestBody {
    pub url: String,
}

/// POST /api/requests — Submit a new URL for download.
pub async fn create_request(
    State(state): State<AppState>,
    Json(body): Json<CreateRequestBody>,
) -> Result<(StatusCode, Json<DownloadRequest>), (StatusCode, Json<serde_json::Value>)> {
    let url = body.url.trim().to_string();
    if url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "url is required" })),
        ));
    }

    // Insert request into DB
    let request = DownloadRequest::create(&state.db, &url)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to create request" })),
            )
        })?;

    // Send job to download queue
    if let Err(e) = state.job_sender.send(DownloadJob {
        request_id: request.id.clone(),
        url: url.clone(),
    }) {
        error!(error = %e, "Failed to enqueue download job");
        let _ = DownloadRequest::update_status(
            &state.db,
            &request.id,
            "failed",
            Some("Failed to enqueue download job"),
        )
        .await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to enqueue download" })),
        ));
    }

    Ok((StatusCode::ACCEPTED, Json(request)))
}

/// GET /api/requests — List all requests (paginated).
pub async fn list_requests(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<DownloadRequest>>, (StatusCode, Json<serde_json::Value>)> {
    let total = DownloadRequest::count(&state.db).await.map_err(|e| {
        error!(error = %e, "Failed to count requests");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Database error" })),
        )
    })?;

    let items = DownloadRequest::list(&state.db, params.per_page(), params.offset())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list requests");
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

/// GET /api/requests/:id — Get a request with its galleries and videos.
pub async fn get_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DownloadRequestDetail>, (StatusCode, Json<serde_json::Value>)> {
    let request = DownloadRequest::get_by_id(&state.db, &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Request not found" })),
            )
        })?;

    let galleries = Gallery::get_by_request_id(&state.db, &id)
        .await
        .unwrap_or_default();

    let videos = Video::get_by_request_id(&state.db, &id)
        .await
        .unwrap_or_default();

    Ok(Json(DownloadRequestDetail {
        request,
        galleries,
        videos,
    }))
}
