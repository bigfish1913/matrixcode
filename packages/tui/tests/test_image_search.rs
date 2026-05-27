//! Test real image search API calls

use matrixcode_tui::image_utils;

#[tokio::test]
async fn test_real_unsplash_search() {
    // Load env vars
    let _ = dotenvy::from_path("../../.env");

    if std::env::var("UNSPLASH_ACCESS_KEY").is_err() {
        println!("Skipping: UNSPLASH_ACCESS_KEY not set");
        return;
    }

    let results = image_utils::search_unsplash("sunset", 2).await;
    match results {
        Ok(images) => {
            println!("Unsplash found {} images", images.len());
            for img in &images {
                println!("  - {} ({})", img.url, img.platform);
            }
            assert!(!images.is_empty(), "Should find images");
        }
        Err(e) => {
            println!("Unsplash error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_real_pexels_search() {
    let _ = dotenvy::from_path("../../.env");

    if std::env::var("PEXELS_API_KEY").is_err() {
        println!("Skipping: PEXELS_API_KEY not set");
        return;
    }

    let results = image_utils::search_pexels("nature", 2).await;
    match results {
        Ok(images) => {
            println!("Pexels found {} images", images.len());
            for img in &images {
                println!("  - {} ({})", img.url, img.platform);
            }
            assert!(!images.is_empty(), "Should find images");
        }
        Err(e) => {
            println!("Pexels error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_real_pixabay_search() {
    let _ = dotenvy::from_path("../../.env");

    if std::env::var("PIXABAY_API_KEY").is_err() {
        println!("Skipping: PIXABAY_API_KEY not set");
        return;
    }

    let results = image_utils::search_pixabay("forest", 2).await;
    match results {
        Ok(images) => {
            println!("Pixabay found {} images", images.len());
            for img in &images {
                println!("  - {} ({})", img.url, img.platform);
            }
            assert!(!images.is_empty(), "Should find images");
        }
        Err(e) => {
            println!("Pixabay error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_real_search_all() {
    let _ = dotenvy::from_path("../../.env");

    let results = image_utils::search_all("mountain", 3).await;
    match results {
        Ok(images) => {
            println!("Total found {} images from all platforms", images.len());
            for img in &images {
                println!("  - [{}] {} by {}", img.platform, img.url, img.photographer);
            }
            // At least one platform should return results if keys are configured
            if std::env::var("UNSPLASH_ACCESS_KEY").is_ok()
                || std::env::var("PEXELS_API_KEY").is_ok()
                || std::env::var("PIXABAY_API_KEY").is_ok() {
                assert!(!images.is_empty(), "Should find images from at least one platform");
            }
        }
        Err(e) => {
            println!("Search all error: {}", e);
            // If no keys configured, this is expected
            if e.to_string().contains("No image search API keys configured") {
                println!("Expected: no keys configured");
            } else {
                panic!("Unexpected error: {}", e);
            }
        }
    }
}