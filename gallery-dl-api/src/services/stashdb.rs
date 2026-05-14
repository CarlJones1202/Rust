use serde::{Deserialize, Serialize};
use tracing::{error, info};

const STASHDB_GRAPHQL_URL: &str = "https://stashdb.org/graphql";

/// A performer as returned from StashDB search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashDBPerformerSummary {
    pub id: String,
    pub name: String,
    pub disambiguation: Option<String>,
    pub aliases: Vec<String>,
    pub gender: Option<String>,
    pub image_url: Option<String>,
}

/// Full performer detail from StashDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashDBPerformerDetail {
    pub id: String,
    pub name: String,
    pub disambiguation: Option<String>,
    pub aliases: Vec<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,
    pub country: Option<String>,
    pub height: Option<i32>,
    pub hair_color: Option<String>,
    pub eye_color: Option<String>,
    pub measurements: Option<StashDBMeasurements>,
    pub breast_type: Option<String>,
    pub career_start_year: Option<i32>,
    pub career_end_year: Option<i32>,
    pub images: Vec<StashDBImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashDBMeasurements {
    pub band_size: Option<i32>,
    pub cup_size: Option<String>,
    pub waist: Option<i32>,
    pub hip: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashDBImage {
    pub url: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

/// Search performers on StashDB by name.
pub async fn search_performers(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Vec<StashDBPerformerSummary>, String> {
    let graphql_query = r#"
        query SearchPerformers($term: String!) {
            searchPerformer(term: $term) {
                id
                name
                disambiguation
                aliases
                gender
                images {
                    url
                    width
                    height
                }
            }
        }
    "#;

    let body = serde_json::json!({
        "query": graphql_query,
        "variables": { "term": query }
    });

    let resp = client
        .post(STASHDB_GRAPHQL_URL)
        .header("ApiKey", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, "StashDB request failed");
            format!("StashDB request failed: {}", e)
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        error!(status = %status, body = %text, "StashDB returned error");
        return Err(format!("StashDB returned status {}: {}", status, text));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| {
        error!(error = %e, "Failed to parse StashDB response");
        format!("Failed to parse response: {}", e)
    })?;

    if let Some(errors) = json.get("errors") {
        error!(errors = %errors, "StashDB GraphQL errors");
        return Err(format!("StashDB GraphQL errors: {}", errors));
    }

    let performers = json
        .get("data")
        .and_then(|d| d.get("searchPerformer"))
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    let results: Vec<StashDBPerformerSummary> = performers
        .into_iter()
        .filter_map(|p| {
            let id = p.get("id")?.as_str()?.to_string();
            let name = p.get("name")?.as_str()?.to_string();
            let disambiguation = p.get("disambiguation").and_then(|v| v.as_str()).map(String::from);
            let aliases = p.get("aliases")
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let gender = p.get("gender").and_then(|v| v.as_str()).map(String::from);
            let image_url = p.get("images")
                .and_then(|imgs| imgs.as_array())
                .and_then(|imgs| imgs.first())
                .and_then(|img| img.get("url"))
                .and_then(|u| u.as_str())
                .map(String::from);

            Some(StashDBPerformerSummary {
                id,
                name,
                disambiguation,
                aliases,
                gender,
                image_url,
            })
        })
        .collect();

    info!(count = results.len(), query = %query, "StashDB search returned results");
    Ok(results)
}

/// Get full performer detail from StashDB.
pub async fn get_performer(
    client: &reqwest::Client,
    api_key: &str,
    performer_id: &str,
) -> Result<StashDBPerformerDetail, String> {
    let graphql_query = r#"
        query FindPerformer($id: ID!) {
            findPerformer(id: $id) {
                id
                name
                disambiguation
                aliases
                gender
                ethnicity
                country
                height
                hair_color
                eye_color
                measurements {
                    band_size
                    cup_size
                    waist
                    hip
                }
                breast_type
                career_start_year
                career_end_year
                images {
                    url
                    width
                    height
                }
            }
        }
    "#;

    let body = serde_json::json!({
        "query": graphql_query,
        "variables": { "id": performer_id }
    });

    let resp = client
        .post(STASHDB_GRAPHQL_URL)
        .header("ApiKey", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("StashDB request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("StashDB returned status {}: {}", status, text));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if let Some(errors) = json.get("errors") {
        return Err(format!("StashDB GraphQL errors: {}", errors));
    }

    let p = json
        .get("data")
        .and_then(|d| d.get("findPerformer"))
        .ok_or_else(|| "Performer not found in StashDB response".to_string())?;

    let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let disambiguation = p.get("disambiguation").and_then(|v| v.as_str()).map(String::from);
    let aliases = p.get("aliases")
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let gender = p.get("gender").and_then(|v| v.as_str()).map(String::from);
    let ethnicity = p.get("ethnicity").and_then(|v| v.as_str()).map(String::from);
    let country = p.get("country").and_then(|v| v.as_str()).map(String::from);
    let height = p.get("height").and_then(|v| v.as_i64()).map(|v| v as i32);
    let hair_color = p.get("hair_color").and_then(|v| v.as_str()).map(String::from);
    let eye_color = p.get("eye_color").and_then(|v| v.as_str()).map(String::from);
    let breast_type = p.get("breast_type").and_then(|v| v.as_str()).map(String::from);
    let career_start_year = p.get("career_start_year").and_then(|v| v.as_i64()).map(|v| v as i32);
    let career_end_year = p.get("career_end_year").and_then(|v| v.as_i64()).map(|v| v as i32);

    let measurements = p.get("measurements").and_then(|m| {
        if m.is_null() { return None; }
        Some(StashDBMeasurements {
            band_size: m.get("band_size").and_then(|v| v.as_i64()).map(|v| v as i32),
            cup_size: m.get("cup_size").and_then(|v| v.as_str()).map(String::from),
            waist: m.get("waist").and_then(|v| v.as_i64()).map(|v| v as i32),
            hip: m.get("hip").and_then(|v| v.as_i64()).map(|v| v as i32),
        })
    });

    let images = p.get("images")
        .and_then(|imgs| imgs.as_array())
        .map(|arr| {
            arr.iter().filter_map(|img| {
                let url = img.get("url")?.as_str()?.to_string();
                let width = img.get("width").and_then(|v| v.as_i64()).map(|v| v as i32);
                let height = img.get("height").and_then(|v| v.as_i64()).map(|v| v as i32);
                Some(StashDBImage { url, width, height })
            }).collect()
        })
        .unwrap_or_default();

    info!(performer_id = %performer_id, name = %name, "Fetched performer from StashDB");

    Ok(StashDBPerformerDetail {
        id,
        name,
        disambiguation,
        aliases,
        gender,
        ethnicity,
        country,
        height,
        hair_color,
        eye_color,
        measurements,
        breast_type,
        career_start_year,
        career_end_year,
        images,
    })
}

/// Download an image from a URL and return the bytes.
pub async fn download_image(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download image: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Image download returned status {}", resp.status()));
    }

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Failed to read image bytes: {}", e))
}
