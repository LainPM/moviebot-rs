use std::sync::Arc;
use thirtyfour::{error::WebDriverResult, By, WebDriver};
use reqwest::Client;
use scraper::{Html, Selector};
use regex::Regex;

pub struct Fasel {
    driver: Arc<WebDriver>,
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
    pub async fn _search(&self, movie_name: &str) -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error>> {
        let client = Client::new();
        let url = format!("https://web184.faselhd.cafe/?s={}", movie_name);
        let res = client.get(&url).send().await?.text().await?;
        
        let document = Html::parse_document(&res);
        let movie_selector = Selector::parse("div.postDiv")?;
        let img_selector = Selector::parse("div.imgdiv-class img")?;
        let title_selector = Selector::parse("div.h1")?;
    
        let movies = document.select(&movie_selector).take(8);
        let images = document.select(&img_selector).take(8);
        
        let mut results = Vec::new();
        let arabic_regex = Regex::new(r"[\u0600-\u06FF]+")?;
    
        for (movie, img) in movies.zip(images) {
            if let Some(a_tag) = movie.select(&Selector::parse("a")?).next() {
                if let Some(href) = a_tag.value().attr("href") {
                    let name = movie.select(&title_selector)
                        .next()
                        .map(|title| title.text().collect::<String>().trim().to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    
                    let cleaned_name = arabic_regex.replace_all(&name, "").to_string();
                    let img_src = img.value().attr("data-src").unwrap_or("").to_string();
                    
                    results.push((href.to_string(), img_src, cleaned_name));
                }
            }
        }
        
        Ok(results)
    }
    
}
