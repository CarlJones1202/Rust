use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use tracing::error;

use crate::models::gallery::Gallery;
use crate::models::request::{DownloadRequest, DownloadRequestDetail};
use crate::models::image::Image;
use crate::models::video::Video;
use crate::pagination::{PaginatedResponse, PaginationMeta, PaginationParams};
use crate::queue::worker::DownloadJob;
use crate::AppState;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct CreateRequestBody {
    pub url: String,
    pub name: Option<String>,
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

    // Check if URL already exists
    if let Ok(Some(_)) = DownloadRequest::get_by_url(&state.db, &url).await {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "URL already exists" })),
        ));
    }

    // Insert request into DB
    let request = DownloadRequest::create(&state.db, &url, body.name.as_deref())
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
        title: request.title.clone(),
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
    let total = DownloadRequest::count(&state.db, params.q.as_deref(), params.status.as_deref())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to count requests");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?;

    let items = DownloadRequest::list(
        &state.db,
        params.per_page(),
        params.offset(),
        params.q.as_deref(),
        params.status.as_deref(),
        params.sort.as_deref(),
    )
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

/// POST /api/requests/:id/requeue — Purge data and restart download.
pub async fn requeue_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<DownloadRequest>), (StatusCode, Json<serde_json::Value>)> {
    // 1. Get the request
    let request = DownloadRequest::get_by_id(&state.db, &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get request for requeue");
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

    // 2. Fetch all media to delete files from disk
    let galleries = Gallery::get_by_request_id(&state.db, &id)
        .await
        .unwrap_or_default();

    for gallery in galleries {
        let images = Image::get_by_gallery_id(&state.db, &gallery.id)
            .await
            .unwrap_or_default();
        for image in images {
            // Only delete file if no other request is using this hash
            let other_count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE i.hash = ? AND g.request_id != ?"
            )
            .bind(&image.hash)
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .unwrap_or((0,));

            if other_count.0 == 0 {
                let path = PathBuf::from(&state.config.storage_dir)
                    .join("images")
                    .join(format!("{}.{}", image.hash, image.extension));
                let _ = tokio::fs::remove_file(path).await;
            }
        }
    }

    let videos = Video::get_by_request_id(&state.db, &id)
        .await
        .unwrap_or_default();
    for video in videos {
        // Only delete file if no other request is using this hash
        let other_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM videos WHERE hash = ? AND request_id != ?"
        )
        .bind(&video.hash)
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        if other_count.0 == 0 {
            let path = PathBuf::from(&state.config.storage_dir)
                .join("videos")
                .join(format!("{}.{}", video.hash, video.extension));
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    // 3. Purge DB records (Galleries and Videos)
    // Cascade will handle images once galleries are deleted
    sqlx::query("DELETE FROM galleries WHERE request_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .ok();
    sqlx::query("DELETE FROM videos WHERE request_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .ok();

    // 4. Reset status
    DownloadRequest::update_status(&state.db, &id, "pending", None)
        .await
        .ok();

    // 5. Enqueue job
    if let Err(e) = state.job_sender.send(DownloadJob {
        request_id: id.clone(),
        url: request.url.clone(),
        title: request.title.clone(),
    }) {
        error!(error = %e, "Failed to re-enqueue download job");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to re-enqueue download" })),
        ));
    }

    // 6. Return the updated request
    let updated_request = DownloadRequest::get_by_id(&state.db, &id)
        .await
        .map_err(|_| {
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

    Ok((StatusCode::ACCEPTED, Json(updated_request)))
}

/// POST /api/requests/nuke — Purge all media and reset every request to pending.
/// StashDB profile picture redownloads run in the background so the API responds immediately.
pub async fn nuke_all(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let count = crate::reset_checker::run_requeue_all(&state.db, &state.config).await;

    let db = state.db.clone();
    let config = state.config.clone();
    let http_client = state.config.http_client.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            crate::reset_checker::redownload_stashdb_images(&db, &config, &http_client).await;
        });
    });

    Ok(Json(serde_json::json!({
        "message": "All media purged and requests requeued.",
        "requeued_count": count,
    })))
}

#[derive(Debug, Deserialize)]
pub struct GuessTitleParams {
    pub url: String,
}

/// GET /api/requests/guess-title — Suggest a title for a given URL.
pub async fn guess_request_title(
    State(state): State<AppState>,
    Query(params): Query<GuessTitleParams>,
) -> Json<serde_json::Value> {
    let title = crate::services::title_guesser::guess_title(&state.db, &params.url).await;
    Json(serde_json::json!({ "title": title }))
}

/// DELETE /api/requests/:id — Delete a request and all its media.
pub async fn delete_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let _request = DownloadRequest::get_by_id(&state.db, &id)
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

    let storage = PathBuf::from(&state.config.storage_dir);

    let galleries = Gallery::get_by_request_id(&state.db, &id)
        .await
        .unwrap_or_default();

    for gallery in &galleries {
        let images = Image::get_by_gallery_id(&state.db, &gallery.id)
            .await
            .unwrap_or_default();

        for image in &images {
            let other_count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE i.hash = ? AND g.request_id != ?"
            )
            .bind(&image.hash)
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .unwrap_or((0,));

            if other_count.0 == 0 {
                let _ = tokio::fs::remove_file(
                    storage.join("images").join(format!("{}.{}", image.hash, image.extension))
                ).await;
                let _ = tokio::fs::remove_file(
                    storage.join("thumbnails").join(format!("{}.jpg", image.hash))
                ).await;
            }
        }
    }

    let videos = Video::get_by_request_id(&state.db, &id)
        .await
        .unwrap_or_default();

    for video in &videos {
        let other_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM videos WHERE hash = ? AND request_id != ?"
        )
        .bind(&video.hash)
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        if other_count.0 == 0 {
            let _ = tokio::fs::remove_file(
                storage.join("videos").join(format!("{}.{}", video.hash, video.extension))
            ).await;
            let _ = tokio::fs::remove_file(
                storage.join("thumbnails").join(format!("{}.jpg", video.hash))
            ).await;
            let _ = tokio::fs::remove_file(
                storage.join("trickplay").join(format!("{}.jpg", video.hash))
            ).await;
        }
    }

    sqlx::query("DELETE FROM requests WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error" })),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}
