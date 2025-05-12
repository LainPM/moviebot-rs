use std::sync::Arc;
use serde_json::json;
use thirtyfour::{error::WebDriverResult, By, WebDriver};
use reqwest::Client;
use scraper::{Html, Selector};
use regex::Regex;
use crate::server::functions::DRIVER_INSTANCE;

use super::netflix::ShowResult;
pub struct Fasel {
    driver: Arc<WebDriver>,
}
pub struct FaselSearcher;

impl FaselSearcher {
    pub async fn _search(movie_name: &str) -> Result<Vec<(String, String, String, ShowResult)>, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::new();
        let url = format!("https://web184.faselhd.cafe/?s={}", movie_name);
        let res = client.get(&url).send().await?.text().await?;
        
        let document = Html::parse_document(&res);
        let movie_selector = Selector::parse("div.postDiv")
        .map_err(|e| format!("Selector parse error: {:?}", e))?;
    
    let img_selector = Selector::parse("div.imgdiv-class img")
        .map_err(|e| format!("Selector parse error: {:?}", e))?;
    
    let title_selector = Selector::parse("div.h1")
        .map_err(|e| format!("Selector parse error: {:?}", e))?;
    
        let movies = document.select(&movie_selector).take(9);
        let images = document.select(&img_selector).take(9);
        
        let mut results = Vec::new();
        let arabic_regex = Regex::new(r"[\u0600-\u06FF]+")?;
    
        let a_selector = Selector::parse("a")
        .map_err(|e| format!("Selector parse error: {:?}", e))?;
    
        for (movie, img) in movies.zip(images) {
            if let Some(a_tag) = movie.select(&a_selector).next() {
                if let Some(href) = a_tag.value().attr("href") {
                    let name = movie
                        .select(&title_selector)
                        .next()
                        .map(|title| title.text().collect::<String>().trim().to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
        
                    let cleaned_name = arabic_regex.replace_all(&name, "").to_string();
                    let img_src = img.value().attr("data-src").unwrap_or("").to_string();
        
                    results.push((href.to_string(), img_src, cleaned_name, ShowResult { show_data: json!({ "movie_id": "0"}), is_show: false}));
                }
            }
        }
        
        Ok(results)
    }
}
impl Fasel {
    pub async fn new(driver: WebDriver) -> Self {
        Fasel {
            driver: Arc::new(driver),
        }
    }
    pub async fn start(&self, url: &str) -> WebDriverResult<()>{
        self.driver.goto(url).await?;
        self.driver.set_implicit_wait_timeout(std::time::Duration::from_secs(10)).await?;
        self.driver.execute("window.scrollTo(0, 1100);", Vec::new()).await?;
        self.driver.enter_frame(0).await?;
        self.driver.find(By::XPath("/html/body/div[1]/div[2]/div[13]/div[1]/div/div/div[2]/div")).await?.click().await?;
        self.driver.execute("document.querySelector(\"#player > div.jw-wrapper.jw-reset > div.jw-controls.jw-reset > div.jw-controlbar.jw-reset > div.jw-reset.jw-button-container > div:nth-child(18)\").click()", Vec::new()).await?;
        Ok(())
    }
    pub async fn pause() -> WebDriverResult<()> {
        let driver_lock = DRIVER_INSTANCE.lock().await;
    
        if let Some(driver) = driver_lock.as_ref() {
            driver.goto("https://google.com").await?;
        } else {
            println!("Nice")
        }
    
        Ok(())
    }
}

