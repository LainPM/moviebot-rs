use std::sync::Arc;

use reqwest::Client;
use serde_json::Value;
use thirtyfour::WebDriver;

use serde::{Deserialize, Serialize};

use super::netflix::ShowResult;

pub struct Shahid {
    driver: Arc<WebDriver>
}

pub struct ShahidSearcher;

impl ShahidSearcher {
    pub async fn search(movie_name: &str) -> Result<Vec<(String, String, String, ShowResult)>, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::new();

        let request_data = serde_json::json!({
            "name": movie_name,
            "pageNumber": 0,
            "pageSize": 24
        });
        let json_request = serde_json::to_string(&request_data)?;

        // ✅ Encode the JSON string using `urlencoding`
        let encoded_request = urlencoding::encode(&json_request);
    
        // Construct API URL
        let url = format!(
            "https://api3.shahid.net/proxy/v2.1/t-search?request={}&exactMatch=false&country=EG",
            encoded_request
        );
    
        // Send request
        let response = client
            .get(&url)
            .header("accept", "application/json")
            .header("accept-language", "en")
            .header("language", "EN")
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
    
        let products = response["productList"]["products"]
            .as_array()
            .ok_or("No products found")?;
    
        let mut results = Vec::new();
        for product in products {
            let title = product["title"].as_str().unwrap_or("Unknown").to_string();
            let playlist_id = product.get("season")
            .and_then(|season| season.get("playlists"))
            .and_then(|playlists| playlists.as_array())
            .and_then(|playlists| 
                playlists.iter()
                    .find(|playlist| match playlist.get("title") {
                        Some(it) => it,
                        None => return false,
                    }.as_str() == Some("Episodes"))
                    .and_then(|playlist| playlist.get("id")?.as_str().map(String::from))
            )
            .unwrap_or("No ID found".to_string());
            let product_type = product["type"].as_str().unwrap_or("").to_string();
    
            // Construct ShowResult
            let show_result = ShowResult {
                show_data: product.clone(), 
                is_show: product["type"].as_str().unwrap_or("") == "show", 
            };
    
            results.push((title, playlist_id, product_type, show_result));
        }
       
        Ok(results)
    }
    pub async fn fetch_shahid_playlist(
        playlist_id: &str,
    ) -> Result<Vec<(String, String)>, reqwest::Error> {
        let client = Client::new();
        let mut page_number = 0;
        let mut results = Vec::new();
    
        loop {
            let playlist_url = Self::format_shahid_playlist_url(playlist_id, page_number).await;
            let response = client.get(&playlist_url)
                .header("User-Agent", "Mozilla/5.0")
                .header("language", "EN")
                .send()
                .await?;
            if !response.status().is_success() {
                eprintln!("Error fetching playlist: HTTP {}", response.status());
                break;
            }
    
            let data: Value = response.json().await?;
            let products = data.get("productList")
                .and_then(|pl| pl.get("products"))
                .and_then(|p| p.as_array());
    
            match products {
                Some(products) if !products.is_empty() => {
                    for product in products {
                        let title = product.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
                        let product_url = product.get("productUrl")
                            .and_then(|url_obj| url_obj.get("url"))
                            .and_then(|url| url.as_str())
                            .unwrap_or("")
                            .to_string();
    
    
                        results.push((product_url, title));
                    }
    
                    page_number += 1;
                }
                _ => {
                    println!("No more results. Stopping pagination.");
                    break;
                }
            }
        }
        println!("{:#?}", results);
        Ok(results)
    }
    async fn format_shahid_playlist_url(playlist_id: &str, page_number: usize) -> String {
        let request_object = serde_json::json!({
            "pageNumber": page_number,
            "pageSize": 6,
            "playListId": playlist_id,
            "sorts": [{ "order": "DESC", "type": "SORTDATE" }],
            "isDynamicPlaylist": false
        });
    
        let request_str = request_object.to_string(); // Store the string to avoid temporary value issue
        let encoded_request = urlencoding::encode(&request_str); // Now it has a valid reference
            format!(
            "https://api3.shahid.net/proxy/v2.1/product/playlist?request={}&country=EG",
            encoded_request
        )
    }
}