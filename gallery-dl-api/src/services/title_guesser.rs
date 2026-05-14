use regex::Regex;
use sqlx::SqlitePool;
use percent_encoding::percent_decode_str;
use std::sync::OnceLock;

static RE_SLUG: OnceLock<Regex> = OnceLock::new();
static RE_PARENS: OnceLock<Regex> = OnceLock::new();
static RE_DATE_INLINE: OnceLock<Regex> = OnceLock::new();
static RE_DATE_INLINE_LONG: OnceLock<Regex> = OnceLock::new();
static RE_DATE_END_1: OnceLock<Regex> = OnceLock::new();
static RE_DATE_END_2: OnceLock<Regex> = OnceLock::new();
static RE_DATE_END_3: OnceLock<Regex> = OnceLock::new();
static RE_DATE_MID: OnceLock<Regex> = OnceLock::new();
static RE_OT_DATE: OnceLock<Regex> = OnceLock::new();
static RE_DATE_END_4: OnceLock<Regex> = OnceLock::new();
static RE_COMPOUND_PHOTOS: OnceLock<Regex> = OnceLock::new();
static RE_COMPOUND_PICTURES: OnceLock<Regex> = OnceLock::new();
static RE_COMPOUND_PICS: OnceLock<Regex> = OnceLock::new();
static RE_COMPOUND_PHOTOS_START: OnceLock<Regex> = OnceLock::new();
static RE_MULTIPLE_HYPHENS: OnceLock<Regex> = OnceLock::new();
static RE_YYYY: OnceLock<Regex> = OnceLock::new();
static RE_MM_OR_DD: OnceLock<Regex> = OnceLock::new();
static RE_MM_OR_DD_OPT_PAREN: OnceLock<Regex> = OnceLock::new();
static RE_YY_OR_MM: OnceLock<Regex> = OnceLock::new();
static RE_YY_OR_YYYY_OPT_PAREN: OnceLock<Regex> = OnceLock::new();
static RE_X_COUNT: OnceLock<Regex> = OnceLock::new();
static RE_PX: OnceLock<Regex> = OnceLock::new();
static RE_DIMENSIONS: OnceLock<Regex> = OnceLock::new();
static RE_CARD_ID: OnceLock<Regex> = OnceLock::new();
static RE_P_RES: OnceLock<Regex> = OnceLock::new();

fn get_regex(lock: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    lock.get_or_init(|| Regex::new(pattern).unwrap())
}

pub async fn guess_title(pool: &SqlitePool, url: &str) -> Option<String> {
    if !url.contains("vipergirls.to/threads/") {
        return None;
    }

    let re_slug = get_regex(&RE_SLUG, r"/threads/\d+-(.*?)(?:\?|#|&|$)");
    let slug = re_slug.captures(url)?.get(1)?.as_str();

    // URL-decode
    let slug = percent_decode_str(slug).decode_utf8_lossy().into_owned();

    // Phase 0: Pre-clean
    let slug = preclean_slug(&slug);

    // Split on hyphens
    let mut parts: Vec<String> = slug.split('-').map(|s| s.to_string()).collect();

    // Phase 1: Strip leading date
    parts = strip_leading_date(parts);

    // Phase 2: Strip site prefixes
    parts = strip_site_prefix(parts);

    // Phase 3: Remove noise
    let mut cleaned = remove_noise(parts);

    // Phase 4: Strip trailing numeric remnants
    cleaned = strip_trailing_numbers(cleaned);

    // Phase 5: Format title (incorporating model names from DB)
    Some(format_title(pool, cleaned).await)
}

fn preclean_slug(slug: &str) -> String {
    let mut s = slug.to_string();

    s = get_regex(&RE_PARENS, r"\([^)]*\)").replace_all(&s, "").into_owned();
    s = get_regex(&RE_DATE_INLINE, r"-(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)-\d{1,2}-\d{4}").replace_all(&s, "").into_owned();
    s = get_regex(&RE_DATE_INLINE_LONG, r"-(?:January|February|March|April|May|June|July|August|September|October|November|December)-\d{1,2}-\d{4}").replace_all(&s, "").into_owned();
    s = get_regex(&RE_DATE_END_1, r"-\d{1,2}-\d{1,2}-\d{2,4}$").replace_all(&s, "").into_owned();
    s = get_regex(&RE_DATE_END_2, r"-[A-Z][a-z]{2}-\d{1,2}-\d{2,4}$").replace_all(&s, "").into_owned();
    s = get_regex(&RE_DATE_END_3, r"-\d{4}-\d{2}-\d{2}$").replace_all(&s, "").into_owned();
    s = get_regex(&RE_DATE_MID, r"-[A-Z][a-z]{2,8}-\d{1,2}-\d{4}").replace_all(&s, "").into_owned();
    s = get_regex(&RE_OT_DATE, r"-Ot-\d{1,2}-\d{4}").replace_all(&s, "").into_owned();
    s = get_regex(&RE_DATE_END_4, r"-\d{2}-\d{2}-\d{4}$").replace_all(&s, "").into_owned();
    
    s = get_regex(&RE_COMPOUND_PHOTOS, r"(?i)-\d+Photos\b").replace_all(&s, "").into_owned();
    s = get_regex(&RE_COMPOUND_PICTURES, r"(?i)-\d+pictures\b").replace_all(&s, "").into_owned();
    s = get_regex(&RE_COMPOUND_PICS, r"(?i)-\d+pics\b").replace_all(&s, "").into_owned();
    s = get_regex(&RE_COMPOUND_PHOTOS_START, r"(?i)\b\d+Photos-").replace_all(&s, "-").into_owned();

    s = get_regex(&RE_MULTIPLE_HYPHENS, r"-{2,}").replace_all(&s, "-").into_owned();
    s.trim_matches('-').to_string()
}

fn strip_leading_date(parts: Vec<String>) -> Vec<String> {
    if parts.len() < 4 {
        return parts;
    }

    let first = parts[0].trim_matches(|c| c == '(' || c == ')');

    // YYYY-MM-DD
    if get_regex(&RE_YYYY, r"^\d{4}$").is_match(first) &&
       get_regex(&RE_MM_OR_DD, r"^\d{2}$").is_match(&parts[1]) &&
       get_regex(&RE_MM_OR_DD_OPT_PAREN, r"^\d{2}\)?$").is_match(&parts[2]) {
        return parts[3..].to_vec();
    }

    // YY-MM-DD or DD-MM-YYYY
    if get_regex(&RE_YY_OR_MM, r"^\d{2}$").is_match(first) &&
       get_regex(&RE_MM_OR_DD, r"^\d{2}$").is_match(&parts[1]) &&
       get_regex(&RE_YY_OR_YYYY_OPT_PAREN, r"^\d{2,4}\)?$").is_match(&parts[2]) {
        
        let rest_start = parts.get(3).map(|s| s.as_str()).unwrap_or("");
        if !rest_start.is_empty() && rest_start.chars().next().unwrap().is_uppercase() {
             return parts[3..].to_vec();
        }
    }

    parts
}

fn strip_site_prefix(parts: Vec<String>) -> Vec<String> {
    if parts.is_empty() {
        return parts;
    }

    if parts[0] == "FemJoy" && parts.len() > 1 && parts[1].to_lowercase() == "com" {
        return parts[2..].to_vec();
    }

    let prefixes = ["Hegre", "Unpublished", "Femjoy"];
    if prefixes.contains(&parts[0].as_str()) && parts.len() > 1 {
        return parts[1..].to_vec();
    }

    parts
}

fn is_noise(token: &str) -> bool {
    let clean = token.trim_matches(|c| c == '(' || c == ')');
    if clean.is_empty() {
        return true;
    }

    let noise_words = [
        "pictures", "photos", "pics", "images", "pix",
        "jpg", "mb", "hi", "res", "pre", "release",
        "upcoming", "full", "set", "card",
    ];
    if noise_words.contains(&clean.to_lowercase().as_str()) {
        return true;
    }

    if get_regex(&RE_X_COUNT, r"(?i)^x\d+$|^\d+x$").is_match(clean) {
        return true;
    }

    if get_regex(&RE_PX, r"(?i)^\d+px$").is_match(clean) {
        return true;
    }

    if get_regex(&RE_DIMENSIONS, r"(?i)^\d+x\d+(px)?$").is_match(clean) {
        return true;
    }

    if clean == "X" || clean == "x" {
        return true;
    }

    if let Ok(num) = clean.parse::<i32>() {
        if num >= 10 {
            return true;
        }
    }

    let months = [
        "Jan", "Feb", "Mar", "Apr", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        "January", "February", "March", "April", "June", "July", "August",
        "September", "October", "November", "December"
    ];
    if months.contains(&clean) {
        return true;
    }

    if get_regex(&RE_CARD_ID, r"^[ef]\d+$").is_match(clean) {
        return true;
    }

    if get_regex(&RE_P_RES, r"^\d+p$").is_match(clean) {
        return true;
    }

    if clean == "*" || clean == "–" || clean == "\u{2013}" || clean == "MP" {
        return true;
    }

    false
}

fn remove_noise(parts: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let token = &parts[i];
        let clean = token.trim_matches(|c| c == '(' || c == ')');
        
        if clean.is_empty() {
            i += 1;
            continue;
        }

        if clean == "May" {
            let prev_is_num = i > 0 && {
                let p = parts[i-1].trim_matches(|c| c == '(' || c == ')');
                p.chars().all(|c| c.is_digit(10))
            };
            let next_is_num = i + 1 < parts.len() && {
                let n = parts[i+1].trim_matches(|c| c == '(' || c == ')');
                n.chars().all(|c| c.is_digit(10))
            };
            if prev_is_num || next_is_num {
                i += 1;
                continue;
            }
            result.push(clean.to_string());
            i += 1;
            continue;
        }

        if is_noise(token) {
            i += 1;
            continue;
        }

        if clean == "amp" {
            result.push("&".to_string());
            i += 1;
            continue;
        }

        result.push(clean.to_string());
        i += 1;
    }
    result
}

fn strip_trailing_numbers(mut parts: Vec<String>) -> Vec<String> {
    while !parts.is_empty() {
        let last = &parts[parts.len() - 1];
        if last.len() <= 2 && last.chars().all(|c| c.is_digit(10)) {
            if parts.len() >= 2 {
                let prev = parts[parts.len() - 2].to_lowercase();
                let keep_words = [
                    "part", "vol", "volume", "chapter", "set", "ii", "iii",
                    "door", "circles", "rambler", "life", "overdrive"
                ];
                if keep_words.contains(&prev.as_str()) {
                    break;
                }

                if let Ok(num) = last.parse::<i32>() {
                    if num <= 4 && parts[parts.len() - 2].chars().next().unwrap_or(' ').is_uppercase() {
                        break;
                    }
                }
            }
            parts.pop();
        } else {
            break;
        }
    }
    parts
}

async fn fetch_known_models(pool: &SqlitePool) -> Vec<String> {
    // Fetch all names and aliases
    let names: Vec<(String,)> = sqlx::query_as("SELECT name FROM persons")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    
    let aliases: Vec<(String,)> = sqlx::query_as("SELECT alias FROM person_aliases")
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let mut all: Vec<String> = names.into_iter().map(|(n,)| n)
        .chain(aliases.into_iter().map(|(a,)| a))
        .filter(|s| s.len() > 1 && !s.chars().all(|c| c.is_digit(10))) // Exclude single letter and purely numeric
        .collect();

    all.sort_by_key(|s| std::cmp::Reverse(s.len()));
    all.dedup();
    all
}

async fn find_model_name(pool: &SqlitePool, parts: &[String]) -> (Option<String>, usize) {
    let models = fetch_known_models(pool).await;
    let joined = parts.join(" ");

    // Exact match first
    for model in &models {
        if joined.starts_with(model) {
            let model_parts: Vec<&str> = model.split(' ').collect();
            if model_parts.len() <= parts.len() {
                if model_parts.iter().enumerate().all(|(i, &mp)| parts[i] == mp) {
                    return (Some(model.clone()), model_parts.len());
                }
            }
        }
    }

    // Case-insensitive fallback
    let joined_lower = joined.to_lowercase();
    for model in &models {
        if joined_lower.starts_with(&model.to_lowercase()) {
            let model_parts: Vec<&str> = model.split(' ').collect();
            if model_parts.len() <= parts.len() {
                if model_parts.iter().enumerate().all(|(i, &mp)| parts[i].to_lowercase() == mp.to_lowercase()) {
                    return (Some(model.clone()), model_parts.len());
                }
            }
        }
    }

    (None, 0)
}

fn is_model_name_word(word: &str) -> bool {
    let clean = word.trim_matches(|c| c == '(' || c == ')');
    if clean.is_empty() {
        return false;
    }
    let first_char = clean.chars().next().unwrap();
    first_char.is_uppercase() && clean.chars().all(|c| c.is_alphabetic())
}

async fn format_title(pool: &SqlitePool, parts: Vec<String>) -> String {
    if parts.is_empty() {
        return "Unknown".to_string();
    }

    let (model_name, consumed) = find_model_name(pool, &parts).await;

    if let Some(model) = model_name {
        if consumed < parts.len() {
            let title = parts[consumed..].join(" ");
            return format!("{} - {}", model, title);
        } else {
            return model;
        }
    }

    // Fallback heuristic
    let mut model_parts = Vec::new();
    let mut title_start = 0;

    for (i, part) in parts.iter().enumerate() {
        if is_model_name_word(part) && i < 4 {
            model_parts.push(part.clone());
            title_start = i + 1;
        } else {
            break;
        }
    }

    if !model_parts.is_empty() && title_start < parts.len() {
        let model = model_parts.join(" ");
        let title = parts[title_start..].join(" ");
        format!("{} - {}", model, title)
    } else {
        parts.join(" ")
    }
}
