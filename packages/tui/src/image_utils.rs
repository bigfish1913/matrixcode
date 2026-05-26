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
#[derive(Debug, Serialize, Clone)]
pub struct ImageResult {
    pub id: String,
    pub url: String,
    pub full_url: String,
    pub thumb_url: String,
    pub description: String,
    pub photographer: String,
    pub photographer_url: String,
    pub width: u32,
    pub height: u32,
    pub platform: String,
    /// URL availability validated
    pub validated: bool,
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

/// Validate image URL accessibility (HEAD request)
async fn validate_image_url(client: &reqwest::Client, url: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    
    match client.head(url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() || status.as_u16() == 302 {
                // Check content-type if available
                if let Some(content_type) = resp.headers().get("content-type") {
                    if let Ok(ct) = content_type.to_str() {
                        return ct.starts_with("image/");
                    }
                }
                // Some servers don't return content-type for HEAD, assume valid
                return status.is_success();
            }
            false
        }
        Err(e) => {
            log::debug!("URL validation failed for {}: {}", url, e);
            false
        }
    }
}

/// Validate image URL with GET request (fallback when HEAD fails)
async fn validate_image_url_get(client: &reqwest::Client, url: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    
    // Only fetch first 1KB to verify it's accessible
    match client
        .get(url)
        .header("Range", "bytes=0-1023")
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            status.is_success() || status.as_u16() == 206 // Partial Content
        }
        Err(_) => false,
    }
}

/// Search Unsplash API
pub async fn search_unsplash(query: &str, per_page: u32) -> Result<Vec<ImageResult>> {
    let key = get_unsplash_key().ok_or_else(|| {
        anyhow::anyhow!("UNSPLASH_ACCESS_KEY not set in environment")
    })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; MatrixCode/1.0)")
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
        validated: false, // Will be validated later
    }).collect())
}

/// Search Pexels API
pub async fn search_pexels(query: &str, per_page: u32) -> Result<Vec<ImageResult>> {
    let key = get_pexels_key().ok_or_else(|| {
        anyhow::anyhow!("PEXELS_API_KEY not set in environment")
    })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; MatrixCode/1.0)")
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
        validated: false,
    }).collect())
}

/// Search Pixabay API
pub async fn search_pixabay(query: &str, per_page: u32) -> Result<Vec<ImageResult>> {
    let key = get_pixabay_key().ok_or_else(|| {
        anyhow::anyhow!("PIXABAY_API_KEY not set in environment")
    })?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; MatrixCode/1.0)")
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
            validated: false,
        }
    }).collect())
}

/// Validate all image URLs concurrently and filter out invalid ones
/// Returns only images with validated URLs
async fn validate_images(images: Vec<ImageResult>) -> Vec<ImageResult> {
    if images.is_empty() {
        return images;
    }
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Mozilla/5.0 (compatible; MatrixCode/1.0)")
        .danger_accept_invalid_certs(true) // Some CDNs may have cert issues
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    
    log::info!("Validating {} image URLs...", images.len());
    
    // Validate all images concurrently
    let mut tasks = Vec::new();
    for img in images {
        let client = client.clone();
        let url = img.url.clone();
        
        let task = tokio::spawn(async move {
            // Try HEAD first (faster)
            let valid = validate_image_url(&client, &url).await;
            
            // If HEAD fails (some servers don't support HEAD), try GET
            let valid = if !valid {
                log::debug!("HEAD failed for {}, trying GET...", url);
                validate_image_url_get(&client, &url).await
            } else {
                true
            };
            
            if valid {
                log::debug!("URL validated: {}", url);
            } else {
                log::debug!("URL invalid: {}", url);
            }
            
            (img, valid)
        });
        
        tasks.push(task);
    }
    
    // Execute all validations concurrently
    let results = futures_util::future::join_all(tasks).await;
    
    // Filter and return only validated images
    let mut validated = Vec::new();
    let total_count = results.len();
    
    for result in results {
        if let Ok((mut img, valid)) = result {
            if valid {
                img.validated = true;
                validated.push(img);
            }
        }
    }
    
    log::info!("Validated {}/{} images", validated.len(), total_count);
    validated
}

/// Search all platforms and return combined results
/// Only searches platforms that have API keys configured
/// Validates all image URLs before returning
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

    for e in &errors {
        log::warn!("Image search partial error: {}", e);
    }

    // Validate all image URLs
    let total_count = all_results.len();
    let validated_results = validate_images(all_results).await;
    
    // Return error if all images failed validation
    if validated_results.is_empty() && !errors.is_empty() {
        return Err(anyhow::anyhow!(
            "All {} images failed URL validation. Errors: {}",
            total_count,
            errors.join("; ")
        ));
    }

    Ok(validated_results)
}