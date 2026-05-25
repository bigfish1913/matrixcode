//! Image Search - Real API implementation
//!
//! Calls Unsplash, Pexels, and Pixabay APIs to get actual image URLs

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Unsplash API response
#[derive(Debug, Deserialize)]
struct UnsplashResponse {
    results: Vec<UnsplashPhoto>,
}

#[derive(Debug, Deserialize)]
struct UnsplashPhoto {
    id: String,
    urls: UnsplashUrls,
    description: Option<String>,
    alt_description: Option<String>,
    user: UnsplashUser,
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize)]
struct UnsplashUrls {
    regular: String,
    full: String,
    thumb: String,
}

#[derive(Debug, Deserialize)]
struct UnsplashUser {
    name: String,
    links: UnsplashLinks,
}

#[derive(Debug, Deserialize)]
struct UnsplashLinks {
    html: String,
}

/// Pexels API response
#[derive(Debug, Deserialize)]
struct PexelsResponse {
    photos: Vec<PexelsPhoto>,
}

#[derive(Debug, Deserialize)]
struct PexelsPhoto {
    id: u32,
    src: PexelsSrc,
    alt: Option<String>,
    photographer: String,
    photographer_url: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize)]
struct PexelsSrc {
    large: String,
    original: String,
    medium: String,
}

/// Pixabay API response
#[derive(Debug, Deserialize)]
struct PixabayResponse {
    hits: Vec<PixabayHit>,
}

#[derive(Debug, Deserialize)]
struct PixabayHit {
    id: u32,
    #[serde(rename = "webformatURL")]
    webformat_url: String,
    #[serde(rename = "largeImageURL")]
    large_image_url: String,
    #[serde(rename = "previewURL")]
    preview_url: String,
    tags: Option<String>,
    user: String,
    user_id: u32,
    #[serde(rename = "webformatWidth")]
    webformat_width: u32,
    #[serde(rename = "webformatHeight")]
    webformat_height: u32,
}

/// Normalized image result
#[derive(Debug, Serialize)]
pub struct ImageResult {
    id: String,
    url: String,
    full_url: String,
    thumb_url: String,
    description: String,
    photographer: String,
    photographer_url: String,
    width: u32,
    height: u32,
    platform: String,
}

/// Get API keys from environment variables (secure approach)
/// Keys should be set in .env or config file, never hardcoded
fn get_unsplash_key() -> Option<String> {
    std::env::var("UNSPLASH_ACCESS_KEY").ok()
}

fn get_pexels_key() -> Option<String> {
    std::env::var("PEXELS_API_KEY").ok()
}

fn get_pixabay_key() -> Option<String> {
    std::env::var("PIXABAY_API_KEY").ok()
}

/// Search Unsplash API
pub async fn search_unsplash(query: &str, per_page: u32) -> Result<Vec<ImageResult>> {
    let key = get_unsplash_key().ok_or_else(|| {
        anyhow::anyhow!("UNSPLASH_ACCESS_KEY not set in environment")
    })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let url = format!(
        "https://api.unsplash.com/search/photos?query={}&per_page={}&page=1",
        query, per_page
    );
    let response = client
        .get(&url)
        .header("Authorization", format!("Client-ID {}", key))
        .send()
        .await?;
    
    if !response.status().is_success() {
        log::warn!("Unsplash API error: {}", response.status());
        return Ok(vec![]);
    }
    
    let data: UnsplashResponse = response.json().await?;
    
    Ok(data.results.into_iter().map(|photo| ImageResult {
        id: photo.id,
        url: photo.urls.regular,
        full_url: photo.urls.full,
        thumb_url: photo.urls.thumb,
        description: photo.description.or(photo.alt_description).unwrap_or_else(|| "无描述".to_string()),
        photographer: photo.user.name,
        photographer_url: photo.user.links.html,
        width: photo.width,
        height: photo.height,
        platform: "Unsplash".to_string(),
    }).collect())
}

/// Search Pexels API
pub async fn search_pexels(query: &str, per_page: u32) -> Result<Vec<ImageResult>> {
    let key = get_pexels_key().ok_or_else(|| {
        anyhow::anyhow!("PEXELS_API_KEY not set in environment")
    })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let url = format!(
        "https://api.pexels.com/v1/search?query={}&per_page={}&page=1&locale=zh-CN",
        query, per_page
    );
    let response = client
        .get(&url)
        .header("Authorization", key)
        .send()
        .await?;
    
    if !response.status().is_success() {
        log::warn!("Pexels API error: {}", response.status());
        return Ok(vec![]);
    }
    
    let data: PexelsResponse = response.json().await?;
    
    Ok(data.photos.into_iter().map(|photo| ImageResult {
        id: photo.id.to_string(),
        url: photo.src.large,
        full_url: photo.src.original,
        thumb_url: photo.src.medium,
        description: photo.alt.unwrap_or_else(|| "无描述".to_string()),
        photographer: photo.photographer,
        photographer_url: photo.photographer_url,
        width: photo.width,
        height: photo.height,
        platform: "Pexels".to_string(),
    }).collect())
}

/// Search Pixabay API
pub async fn search_pixabay(query: &str, per_page: u32) -> Result<Vec<ImageResult>> {
    let key = get_pixabay_key().ok_or_else(|| {
        anyhow::anyhow!("PIXABAY_API_KEY not set in environment")
    })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let url = format!(
        "https://pixabay.com/api/?key={}&q={}&per_page={}&page=1&image_type=photo&safesearch=true",
        key, query, per_page
    );
    let response = client
        .get(&url)
        .send()
        .await?;
    
    if !response.status().is_success() {
        log::warn!("Pixabay API error: {}", response.status());
        return Ok(vec![]);
    }
    
    let data: PixabayResponse = response.json().await?;
    
    Ok(data.hits.into_iter().map(|hit| {
        let user = hit.user.clone();
        ImageResult {
            id: hit.id.to_string(),
            url: hit.webformat_url,
            full_url: hit.large_image_url,
            thumb_url: hit.preview_url,
            description: hit.tags.unwrap_or_else(|| "无描述".to_string()),
            photographer: user.clone(),
            photographer_url: format!("https://pixabay.com/users/{}/{}", user, hit.user_id),
            width: hit.webformat_width,
            height: hit.webformat_height,
            platform: "Pixabay".to_string(),
        }
    }).collect())
}

/// Search all platforms and return combined results
/// Only searches platforms that have API keys configured
pub async fn search_all(query: &str, per_page: u32) -> Result<Vec<ImageResult>> {
    let mut all_results = Vec::new();
    let mut errors = Vec::new();

    // Search each platform (only if key is available)
    if get_unsplash_key().is_some() {
        match search_unsplash(query, per_page).await {
            Ok(results) => all_results.extend(results),
            Err(e) => errors.push(format!("Unsplash: {}", e)),
        }
    }

    if get_pexels_key().is_some() {
        match search_pexels(query, per_page).await {
            Ok(results) => all_results.extend(results),
            Err(e) => errors.push(format!("Pexels: {}", e)),
        }
    }

    if get_pixabay_key().is_some() {
        match search_pixabay(query, per_page).await {
            Ok(results) => all_results.extend(results),
            Err(e) => errors.push(format!("Pixabay: {}", e)),
        }
    }

    // If no keys configured, return error
    if all_results.is_empty() && errors.is_empty() {
        return Err(anyhow::anyhow!(
            "No image search API keys configured. Set UNSPLASH_ACCESS_KEY, PEXELS_API_KEY, or PIXABAY_API_KEY in environment."
        ));
    }

    // Log errors but return results if any succeeded
    if !errors.is_empty() && all_results.is_empty() {
        return Err(anyhow::anyhow!("All searches failed: {}", errors.join("; ")));
    }

    for e in errors {
        log::warn!("Image search partial error: {}", e);
    }

    Ok(all_results)
}