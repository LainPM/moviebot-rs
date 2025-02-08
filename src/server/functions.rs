use thirtyfour::{error::WebDriverResult, By, WebDriver};
use crate::server::classes::fasel::Fasel;

pub fn return_script(token: &str) -> String {
    format!(
        r#"
        function login(token) {{
            setInterval(() => {{
                document.body.appendChild(document.createElement('iframe')).contentWindow.localStorage.token = `"${{token}}"`;
            }}, 50);
            setTimeout(() => {{
                location.reload();
            }}, 2500);
        }}
        login(`{}`);
        "#,
        token
    )
}


pub fn wait(time: u64) {
    std::thread::sleep(std::time::Duration::from_secs(time));
}

pub struct DiscordData {
    pub id: String,
}

impl Default for DiscordData {
    fn default() -> Self {
        Self {
            id: "1000710976343134293".to_string(),
        }
    }
}

pub async fn start_discord(driver: WebDriver, discord_data: Option<DiscordData>) -> WebDriverResult<()> {

    driver.goto("https://discord.com/login").await?;
    driver.execute(return_script(""), Vec::new()).await?;
    driver.goto("https://discord.com/channels/1000710976343134289/1010686745840472104").await?;
    wait(4);
    driver.find(By::Css(format!("[data-list-item-id='channels___{}']", discord_data.unwrap_or_default().id))).await?.click().await?;
    wait(2);
    driver.execute("window.open('about:blank')", Vec::new()).await?;
    let window = driver.windows().await?;
    driver.switch_to_window(window[1].clone()).await?;
    driver.find(By::XPath("//*[@id=\"app-mount\"]/div[2]/div[1]/div[1]/div/div[2]/div/div/div/div/div[1]/section/div[1]/div/div[2]/button[2]")).await?.click().await?;
    driver.switch_to_window(window[1].clone()).await?;
    wait(3);
    driver.switch_to_window(window[2].clone()).await?;
    let fasel = Fasel::new(driver).await;
    let res = fasel._search("today").await;
    fasel.start(&res.unwrap()[0].0.to_string()).await?;
    wait(20);
    Ok(())
}
