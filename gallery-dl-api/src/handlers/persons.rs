use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use md5::{Digest, Md5};
use std::path::PathBuf;
use tracing::error;

use crate::models::person::{
    gallery_count_for_person, get_persons_for_gallery,
    link_gallery_person, unlink_gallery_person, Person, PersonAlias, PersonDetail,
    PersonImage, PersonInput,
};
use crate::models::gallery::{Gallery, GalleryWithCover};
use crate::pagination::{PaginatedResponse, PaginationMeta, PaginationParams};
use crate::services::auto_link;
use crate::services::stashdb;
use crate::AppState;

type ApiError = (StatusCode, Json<serde_json::Value>);

fn internal_error(msg: &str, e: impl std::fmt::Display) -> ApiError {
    error!(error = %e, "{}", msg);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg })),
    )
}

fn not_found(entity: &str) -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": format!("{} not found", entity) })),
    )
}

// === Person CRUD ===

#[derive(Debug, serde::Deserialize)]
pub struct CreatePersonBody {
    pub name: String,
    pub aliases: Option<Vec<String>>,
}

/// POST /api/persons — Create a new person.
pub async fn create_person(
    State(state): State<AppState>,
    Json(body): Json<CreatePersonBody>,
) -> Result<Json<PersonDetail>, ApiError> {
    let person = Person::create(&state.db, &body.name)
        .await
        .map_err(|e| internal_error("Failed to create person", e))?;

    if let Some(ref aliases) = body.aliases {
        PersonAlias::set_aliases(&state.db, &person.id, aliases)
            .await
            .map_err(|e| internal_error("Failed to set aliases", e))?;
    }

    let aliases = PersonAlias::get_for_person(&state.db, &person.id)
        .await
        .unwrap_or_default();

    // Retro-actively link to any existing galleries whose URL matches this person
    let pool = state.db.clone();
    let person_id = person.id.clone();
    let person_name = body.name.clone();
    tokio::spawn(async move {
        let _ = auto_link::retroactively_link_person(&pool, &person_id, &person_name).await;
    });

    Ok(Json(PersonDetail {
        person,
        aliases,
        images: vec![],
        gallery_count: 0,
    }))
}

/// GET /api/persons — List persons (paginated, searchable).
pub async fn list_persons(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<crate::models::person::PersonSummary>>, ApiError> {
    let search = params.q.as_deref();
    let total = Person::count(&state.db, search)
        .await
        .map_err(|e| internal_error("Failed to count people", e))?;

    let items = Person::list_summaries(&state.db, params.per_page(), params.offset(), search)
        .await
        .map_err(|e| internal_error("Failed to list people", e))?;

    Ok(Json(PaginatedResponse {
        data: items,
        pagination: PaginationMeta::new(params.page(), params.per_page(), total),
    }))
}

/// GET /api/persons/:id — Get person detail.
pub async fn get_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PersonDetail>, ApiError> {
    let person = Person::get_by_id(&state.db, &id)
        .await
        .map_err(|e| internal_error("Failed to get person", e))?
        .ok_or_else(|| not_found("Person"))?;

    let aliases = PersonAlias::get_for_person(&state.db, &id)
        .await
        .unwrap_or_default();
    let images = PersonImage::get_for_person(&state.db, &id)
        .await
        .unwrap_or_default();
    let gallery_count = gallery_count_for_person(&state.db, &id)
        .await
        .unwrap_or(0);

    Ok(Json(PersonDetail {
        person,
        aliases,
        images,
        gallery_count,
    }))
}

/// PATCH /api/persons/:id — Update person metadata.
pub async fn update_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PersonInput>,
) -> Result<Json<PersonDetail>, ApiError> {
    let _existing = Person::get_by_id(&state.db, &id)
        .await
        .map_err(|e| internal_error("Failed to get person", e))?
        .ok_or_else(|| not_found("Person"))?;

    Person::update(&state.db, &id, &body)
        .await
        .map_err(|e| internal_error("Failed to update person", e))?;

    if let Some(ref aliases) = body.aliases {
        PersonAlias::set_aliases(&state.db, &id, aliases)
            .await
            .map_err(|e| internal_error("Failed to set aliases", e))?;
    }

    // Return updated detail
    let person = Person::get_by_id(&state.db, &id).await.unwrap().unwrap();
    let aliases = PersonAlias::get_for_person(&state.db, &id)
        .await
        .unwrap_or_default();
    let images = PersonImage::get_for_person(&state.db, &id)
        .await
        .unwrap_or_default();
    let gallery_count = gallery_count_for_person(&state.db, &id)
        .await
        .unwrap_or(0);

    Ok(Json(PersonDetail {
        person,
        aliases,
        images,
        gallery_count,
    }))
}

/// DELETE /api/persons/:id — Delete a person.
pub async fn delete_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = Person::delete(&state.db, &id)
        .await
        .map_err(|e| internal_error("Failed to delete person", e))?;

    if deleted {
        // Clean up person image files
        let person_dir = PathBuf::from(&state.config.storage_dir).join("persons").join(&id);
        let _ = tokio::fs::remove_dir_all(&person_dir).await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found("Person"))
    }
}

// === Person Images ===

/// POST /api/persons/:id/images — Upload a profile image (multipart).
pub async fn upload_person_image(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<PersonImage>, ApiError> {
    let _person = Person::get_by_id(&state.db, &id)
        .await
        .map_err(|e| internal_error("Failed to get person", e))?
        .ok_or_else(|| not_found("Person"))?;

    let existing_images = PersonImage::get_for_person(&state.db, &id)
        .await
        .unwrap_or_default();
    let is_first = existing_images.is_empty();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        internal_error("Failed to read multipart", e)
    })? {
        if field.name() == Some("image") {
            let filename = field.file_name().unwrap_or("upload.jpg").to_string();
            let extension = filename
                .rsplit('.')
                .next()
                .unwrap_or("jpg")
                .to_lowercase();
            let data = field.bytes().await.map_err(|e| {
                internal_error("Failed to read image data", e)
            })?;

            // Hash the file
            let mut hasher = Md5::new();
            hasher.update(&data);
            let hash = format!("{:x}", hasher.finalize());

            // Store file
            let persons_dir = PathBuf::from(&state.config.storage_dir).join("persons");
            tokio::fs::create_dir_all(&persons_dir).await.map_err(|e| {
                internal_error("Failed to create directory", e)
            })?;
            let file_path = persons_dir.join(format!("{}.{}", hash, extension));
            tokio::fs::write(&file_path, &data).await.map_err(|e| {
                internal_error("Failed to write image file", e)
            })?;

            // Get dimensions
            let (width, height) = if let Ok(img) = image::load_from_memory(&data) {
                (Some(img.width() as i32), Some(img.height() as i32))
            } else {
                (None, None)
            };

            // Generate thumbnail
            let thumb_dir = PathBuf::from(&state.config.storage_dir).join("thumbnails");
            tokio::fs::create_dir_all(&thumb_dir).await.ok();
            let thumb_path = thumb_dir.join(format!("{}.jpg", hash));
            if !thumb_path.exists() {
                if let Ok(img) = image::load_from_memory(&data) {
                    let thumb = img.thumbnail(400, 400);
                    let _ = thumb.save(&thumb_path);
                }
            }

            let record = PersonImage::create(
                &state.db,
                &id,
                &hash,
                &extension,
                width,
                height,
                is_first, // first image is automatically primary
                None,
            )
            .await
            .map_err(|e| internal_error("Failed to save image record", e))?;

            return Ok(Json(record));
        }
    }

    Err((
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "No image field found in upload" })),
    ))
}

/// DELETE /api/persons/:person_id/images/:image_id
pub async fn delete_person_image(
    State(state): State<AppState>,
    Path((_person_id, image_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let deleted = PersonImage::delete(&state.db, &image_id)
        .await
        .map_err(|e| internal_error("Failed to delete image", e))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found("Image"))
    }
}

/// PATCH /api/persons/:person_id/images/:image_id/primary
pub async fn set_primary_image(
    State(state): State<AppState>,
    Path((person_id, image_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    PersonImage::set_primary(&state.db, &person_id, &image_id)
        .await
        .map_err(|e| internal_error("Failed to set primary image", e))?;
    Ok(StatusCode::NO_CONTENT)
}

// === Gallery ↔ Person Linking ===

/// POST /api/persons/:person_id/galleries/:gallery_id
pub async fn link_gallery(
    State(state): State<AppState>,
    Path((person_id, gallery_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    // Verify both exist
    Person::get_by_id(&state.db, &person_id)
        .await
        .map_err(|e| internal_error("DB error", e))?
        .ok_or_else(|| not_found("Person"))?;
    Gallery::get_by_id(&state.db, &gallery_id)
        .await
        .map_err(|e| internal_error("DB error", e))?
        .ok_or_else(|| not_found("Gallery"))?;

    link_gallery_person(&state.db, &gallery_id, &person_id)
        .await
        .map_err(|e| internal_error("Failed to link gallery", e))?;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/persons/:person_id/galleries/:gallery_id
pub async fn unlink_gallery(
    State(state): State<AppState>,
    Path((person_id, gallery_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    unlink_gallery_person(&state.db, &gallery_id, &person_id)
        .await
        .map_err(|e| internal_error("Failed to unlink gallery", e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/persons/:id/relink — Scan all completed requests and auto-link any
/// galleries whose URL person name matches this person or their aliases.
pub async fn relink_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let person = Person::get_by_id(&state.db, &id)
        .await
        .map_err(|e| internal_error("DB error", e))?
        .ok_or_else(|| not_found("Person"))?;

    let aliases = PersonAlias::get_for_person(&state.db, &id)
        .await
        .unwrap_or_default();

    let mut name_variants: Vec<String> = vec![person.name.clone()];
    name_variants.extend(aliases);

    let requests: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, url FROM requests WHERE status = 'completed'"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| internal_error("Failed to fetch requests", e))?;

    let mut linked = 0u64;

    for (req_id, url) in &requests {
        let Some(candidate_name) = crate::services::title_guesser::guess_person_name_from_url(url)
        else {
            continue;
        };

        if !name_variants.iter().any(|v| v.eq_ignore_ascii_case(&candidate_name)) {
            continue;
        }

        let galleries: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM galleries WHERE request_id = ?"
        )
        .bind(req_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| internal_error("Failed to fetch galleries", e))?;

        for (gallery_id,) in &galleries {
            if let Err(e) = link_gallery_person(&state.db, gallery_id, &id).await {
                error!(error = %e, "Failed to link gallery");
            } else {
                linked += 1;
            }
        }
    }

    Ok(Json(serde_json::json!({ "linked": linked })))
}

/// GET /api/persons/:id/galleries — Get galleries linked to a person.
pub async fn get_person_galleries(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<GalleryWithCover>>, ApiError> {
    let galleries = Gallery::get_by_person_id(&state.db, &id)
        .await
        .map_err(|e| internal_error("Failed to get galleries", e))?;

    Ok(Json(galleries))
}

// === StashDB Integration ===

#[derive(Debug, serde::Deserialize)]
pub struct StashDBSearchQuery {
    pub q: String,
}

/// GET /api/stashdb/search?q=name — Search StashDB performers.
pub async fn search_stashdb(
    State(state): State<AppState>,
    Query(params): Query<StashDBSearchQuery>,
) -> Result<Json<Vec<stashdb::StashDBPerformerSummary>>, ApiError> {
    let api_key = state.config.stashdb_api_key.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "StashDB API key not configured" })),
        )
    })?;

    let results = stashdb::search_performers(&state.http_client, api_key, &params.q)
        .await
        .map_err(|e| internal_error("StashDB search failed", e))?;

    Ok(Json(results))
}

#[derive(Debug, serde::Deserialize)]
pub struct StashDBImportBody {
    pub stashdb_id: String,
}

/// POST /api/persons/:id/stashdb-import — Import metadata from StashDB.
pub async fn import_from_stashdb(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<StashDBImportBody>,
) -> Result<Json<PersonDetail>, ApiError> {
    let _person = Person::get_by_id(&state.db, &id)
        .await
        .map_err(|e| internal_error("Failed to get person", e))?
        .ok_or_else(|| not_found("Person"))?;

    let api_key = state.config.stashdb_api_key.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "StashDB API key not configured" })),
        )
    })?;

    // Fetch full performer data
    let performer = stashdb::get_performer(&state.http_client, api_key, &body.stashdb_id)
        .await
        .map_err(|e| internal_error("StashDB fetch failed", e))?;

    // Format measurements string
    let measurements_str = performer.measurements.as_ref().map(|m| {
        let mut parts = Vec::new();
        if let (Some(band), Some(ref cup)) = (m.band_size, &m.cup_size) {
            parts.push(format!("{}{}", band, cup));
        }
        if let Some(waist) = m.waist {
            parts.push(waist.to_string());
        }
        if let Some(hip) = m.hip {
            parts.push(hip.to_string());
        }
        parts.join("-")
    });

    // Update person metadata
    let input = PersonInput {
        name: Some(performer.name.clone()),
        disambiguation: performer.disambiguation.clone(),
        gender: performer.gender.clone(),
        ethnicity: performer.ethnicity.clone(),
        country: performer.country.clone(),
        height: performer.height,
        hair_color: performer.hair_color.clone(),
        eye_color: performer.eye_color.clone(),
        measurements: measurements_str,
        breast_type: performer.breast_type.clone(),
        career_start_year: performer.career_start_year,
        career_end_year: performer.career_end_year,
        bio: None,
        extra_data: None,
        aliases: Some(performer.aliases.clone()),
    };

    Person::update(&state.db, &id, &input)
        .await
        .map_err(|e| internal_error("Failed to update person", e))?;

    PersonAlias::set_aliases(&state.db, &id, &performer.aliases)
        .await
        .map_err(|e| internal_error("Failed to set aliases", e))?;

    Person::set_stashdb_id(&state.db, &id, &body.stashdb_id)
        .await
        .map_err(|e| internal_error("Failed to set stashdb_id", e))?;

    // Download and store performer images
    let persons_dir = PathBuf::from(&state.config.storage_dir).join("persons");
    tokio::fs::create_dir_all(&persons_dir).await.ok();
    let thumb_dir = PathBuf::from(&state.config.storage_dir).join("thumbnails");
    tokio::fs::create_dir_all(&thumb_dir).await.ok();

    for (i, stash_img) in performer.images.iter().enumerate() {
        match stashdb::download_image(&state.http_client, &stash_img.url).await {
            Ok(data) => {
                let mut hasher = Md5::new();
                hasher.update(&data);
                let hash = format!("{:x}", hasher.finalize());
                let extension = stash_img.url
                    .rsplit('.')
                    .next()
                    .unwrap_or("jpg")
                    .split('?')
                    .next()
                    .unwrap_or("jpg")
                    .to_lowercase();
                let extension = if extension.len() > 4 { "jpg".to_string() } else { extension };

                let file_path = persons_dir.join(format!("{}.{}", hash, extension));
                if tokio::fs::write(&file_path, &data).await.is_ok() {
                    // Get dimensions
                    let (w, h) = if let Ok(img) = image::load_from_memory(&data) {
                        // Generate thumbnail
                        let thumb_path = thumb_dir.join(format!("{}.jpg", hash));
                        if !thumb_path.exists() {
                            let thumb = img.thumbnail(400, 400);
                            let _ = thumb.save(&thumb_path);
                        }
                        (Some(img.width() as i32), Some(img.height() as i32))
                    } else {
                        (stash_img.width, stash_img.height)
                    };

                    let _ = PersonImage::create(
                        &state.db,
                        &id,
                        &hash,
                        &extension,
                        w,
                        h,
                        i == 0, // First image is primary
                        Some(&stash_img.url),
                    )
                    .await;
                }
            }
            Err(e) => {
                error!(url = %stash_img.url, error = %e, "Failed to download StashDB image");
            }
        }
    }

    // Return updated detail
    let person = Person::get_by_id(&state.db, &id).await.unwrap().unwrap();
    let aliases = PersonAlias::get_for_person(&state.db, &id).await.unwrap_or_default();
    let images = PersonImage::get_for_person(&state.db, &id).await.unwrap_or_default();
    let gallery_count = gallery_count_for_person(&state.db, &id).await.unwrap_or(0);

    Ok(Json(PersonDetail {
        person,
        aliases,
        images,
        gallery_count,
    }))
}

/// GET /api/galleries/:id/persons — Get persons linked to a gallery.
pub async fn get_gallery_persons(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::models::person::PersonSummary>>, ApiError> {
    let persons = get_persons_for_gallery(&state.db, &id)
        .await
        .map_err(|e| internal_error("Failed to get persons", e))?;
    Ok(Json(persons))
}
