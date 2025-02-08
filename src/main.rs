use thirtyfour::{prelude::*, ChromeCapabilities};
mod server;
use server::functions::{start_discord, DiscordData};
struct Streamer;
#[warn(unused_must_use)]
impl Streamer {
    async fn start(_url: &str, r#_type: &str, _id: &str) -> WebDriverResult<()>  {
        let mut options = ChromeCapabilities::new();
        options.add_arg("--ignore-ssl-errors=yes").unwrap();
        options.add_arg("--ignore-certificate-errors").unwrap();
        options.add_arg(
            "--auto-select-tab-capture-source-by-title=about:blank"
        ).unwrap();
        options.add_arg("--disable-gpu").unwrap();
        options.add_arg("--enable-chrome-browser-cloud-management").unwrap();
        options.add_arg("--enable-javascript").unwrap();
        options.add_arg("--disable-blink-features=AutomationControlled").unwrap();
        options.add_arg("--auto-accept-camera-and-microphone-capture").unwrap();
        options.add_arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0").unwrap();
        options.add_arg("--load-extension=").unwrap();
        
        let driver = WebDriver::new("http://localhost:50000", options).await?;
        start_discord(driver, Some(DiscordData { id: "1000710976343134293".to_string()})).await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> WebDriverResult<()> {
     Streamer::start("", "", "").await?;
     Ok(())
}