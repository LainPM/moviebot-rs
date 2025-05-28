use std::env;
use std::sync::Arc;
use std::time::Duration;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::{GuildId, UserId};
use serenity::prelude::*;

use reqwest;
use serde::Deserialize;
use serde_json;
use scraper::{Html, Selector};

use songbird::input::{HttpRequest, Input};
use songbird::SerenityInit;
use songbird::{Call, Songbird, TrackEvent};
use songbird::Event;
use songbird::EventContext;
use songbird::EventHandler as SongbirdEventHandler;

// TMDB Data Structures
#[derive(Deserialize, Debug, Clone)]
struct SearchResultItem {
    id: i32, // TMDB ID
    title: Option<String>, 
    name: Option<String>,  
    #[serde(default)] 
    media_type: String,
    overview: Option<String>,
    release_date: Option<String>,    
    first_air_date: Option<String>, 
}

#[derive(Deserialize, Debug)]
struct TmdbSearchResponse { 
    results: Vec<SearchResultItem>,
}

#[derive(Deserialize, Debug)]
struct TmdbFindResponse {
    movie_results: Vec<SearchResultItem>,
    tv_results: Vec<SearchResultItem>,
}

#[derive(Deserialize, Debug, Clone)]
struct TmdbSeasonBasics {
    season_number: u32,
    episode_count: u32,
    id: i32, // Season's TMDB ID
    name: Option<String>,
    overview: Option<String>,
    air_date: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct TmdbTvDetails {
    id: i32, // Show's TMDB ID
    name: String,
    number_of_seasons: u32,
    seasons: Vec<TmdbSeasonBasics>,
    #[serde(default)]
    overview: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct TmdbEpisodeInfo {
    episode_number: u32,
    name: String,
    id: i32, // Episode's TMDB ID
    #[serde(default)]
    overview: Option<String>,
    air_date: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct TmdbSeasonDetails {
    episodes: Vec<TmdbEpisodeInfo>,
    id: i32, // Season's TMDB ID (returned by API)
    name: String, // Season name (e.g. "Season 1")
    season_number: u32,
}


// HTTP Client
lazy_static::lazy_static! {
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::new();
}

// TMDB API Helper Functions
async fn get_tmdb_tv_details(api_key: &str, tmdb_tv_id: &str) -> Result<TmdbTvDetails, reqwest::Error> {
    let url = format!(
        "https://api.themoviedb.org/3/tv/{}?api_key={}",
        tmdb_tv_id, api_key
    );
    println!("Fetching TV details from: {}", url);
    let response = HTTP_CLIENT.get(&url).send().await?.error_for_status()?.json::<TmdbTvDetails>().await?;
    Ok(response)
}

async fn get_tmdb_season_details(api_key: &str, tmdb_tv_id: &str, season_number: u32) -> Result<TmdbSeasonDetails, reqwest::Error> {
    let url = format!(
        "https://api.themoviedb.org/3/tv/{}/season/{}?api_key={}",
        tmdb_tv_id, season_number, api_key
    );
    println!("Fetching season details from: {}", url);
    let response = HTTP_CLIENT.get(&url).send().await?.error_for_status()?.json::<TmdbSeasonDetails>().await?;
    Ok(response)
}


// Function to search TMDB by query
async fn search_tmdb(api_key: &str, query: &str) -> Result<Vec<SearchResultItem>, reqwest::Error> {
    let url = format!(
        "https://api.themoviedb.org/3/search/multi?api_key={}&query={}",
        api_key, query
    );
    let response = HTTP_CLIENT.get(&url).send().await?.json::<TmdbSearchResponse>().await?;
    Ok(response.results)
}

// Function to find by IMDB ID using TMDB
async fn find_by_imdb_id(api_key: &str, imdb_id: &str) -> Result<Option<SearchResultItem>, reqwest::Error> {
    let url = format!(
        "https://api.themoviedb.org/3/find/{}?api_key={}&external_source=imdb_id",
        imdb_id, api_key
    );
    let response = HTTP_CLIENT.get(&url).send().await?.json::<TmdbFindResponse>().await?;

    if let Some(mut movie) = response.movie_results.into_iter().next() {
        movie.media_type = "movie".to_string();
        return Ok(Some(movie));
    }
    if let Some(mut tv_show) = response.tv_results.into_iter().next() {
        tv_show.media_type = "tv".to_string();
        return Ok(Some(tv_show));
    }
    Ok(None)
}

// Function to get VidSrc streaming URL
async fn get_vidsrc_streaming_url(
    tmdb_id: &str,
    media_type: &str, // "movie" or "tv"
    season: Option<usize>,
    episode: Option<usize>,
) -> Result<Option<String>, reqwest::Error> {
    let domains = ["https://vidsrc.to", "https://vidsrc.me"];
    let mut url_to_try;

    for domain in domains.iter() {
        url_to_try = match media_type {
            "movie" => format!("{}/embed/movie/{}", domain, tmdb_id),
            "tv" => {
                if let (Some(s), Some(e)) = (season, episode) {
                    format!("{}/embed/tv/{}/{}-{}", domain, tmdb_id, s, e)
                } else {
                    println!("Season and episode are required for TV shows when getting VidSrc URL.");
                    return Ok(None); 
                }
            }
            _ => {
                println!("Invalid media_type for VidSrc: {}", media_type);
                return Ok(None);
            }
        };

        println!("Attempting to fetch VidSrc page: {}", url_to_try);
        match HTTP_CLIENT.get(&url_to_try).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let html_content = response.text().await?;
                    let document = Html::parse_document(&html_content);
                    let iframe_selector = Selector::parse("iframe[src*='2embed'], iframe[src*='vidsrc.me/player'], iframe[src*='vidsrc.xyz/player'], iframe[src*='player.vidsrc.to']").unwrap();
                    
                    if let Some(element) = document.select(&iframe_selector).next() {
                        if let Some(src) = element.value().attr("src") {
                            let mut full_src = src.to_string();
                            if full_src.starts_with("//") {
                                full_src = format!("https:{}",full_src);
                            }
                            println!("Found iframe src: {}", full_src);
                            return Ok(Some(full_src));
                        }
                    } else {
                         println!("No matching iframe found on {}", url_to_try);
                    }
                } else {
                    println!("Request to {} failed with status: {}", url_to_try, response.status());
                }
            }
            Err(e) => {
                println!("Error fetching {}: {:?}", url_to_try, e);
            }
        }
    }
    println!("Failed to find streaming URL from all VidSrc domains for TMDB ID {}.", tmdb_id);
    Ok(None)
}

// Helper function to resolve player URL to a direct media link
async fn resolve_player_url(player_page_url: &str) -> Result<Option<String>, reqwest::Error> {
    println!("Resolving player URL: {}", player_page_url);
    let mut current_url = player_page_url.to_string();
    if !current_url.starts_with("http://") && !current_url.starts_with("https://") {
        current_url = format!("https:{}", current_url);
    }
    
    let response = HTTP_CLIENT.get(&current_url).send().await?;
    if !response.status().is_success() {
        println!("Failed to fetch player page {}: Status {}", current_url, response.status());
        return Ok(None);
    }
    let html_content = response.text().await?;
    let document = Html::parse_document(&html_content);

    if let Some(video_tag) = document.select(&Selector::parse("video").unwrap()).next() {
        if let Some(src) = video_tag.value().attr("src") {
            println!("Found video tag with src: {}", src);
            return Ok(Some(src.to_string()));
        }
        if let Some(source_tag) = video_tag.select(&Selector::parse("source").unwrap()).next() {
            if let Some(src) = source_tag.value().attr("src") {
                println!("Found source tag with src: {}", src);
                return Ok(Some(src.to_string()));
            }
        }
    }
    
    for script in document.select(&Selector::parse("script").unwrap()) {
        let script_text = script.inner_html();
        if let Some(start) = script_text.find("file:\"") {
            if let Some(end) = script_text[start + 6..].find("\"") {
                let potential_url = &script_text[start + 6..start + 6 + end];
                if potential_url.contains(".m3u8") || potential_url.contains(".mp4") {
                     println!("Found potential URL in script: {}", potential_url);
                    return Ok(Some(potential_url.to_string()));
                }
            }
        }
        if let Some(mat) = regex::Regex::new(r#"(https?://[^"]+\.m3u8)"#).unwrap().find(&script_text) {
            println!("Found m3u8 URL via regex in script: {}", mat.as_str());
            return Ok(Some(mat.as_str().to_string()));
        }
    }

    println!("Could not resolve a direct media URL from {}", player_page_url);
    Ok(None)
}

// Songbird Track Event Handler
struct TrackEndNotifier {
    current_track_title: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl SongbirdEventHandler for TrackEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        println!("Track ended.");
        let mut title_guard = self.current_track_title.lock().await;
        *title_guard = None;
        None
    }
}


struct Handler {
    current_track_title: Arc<Mutex<Option<String>>>,
}

impl Handler {
    fn new() -> Self {
        Self {
            current_track_title: Arc::new(Mutex::new(None)),
        }
    }
}


#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let prefix = "m!";
        let alt_prefix = "!";
        let content = &msg.content;

        if content.starts_with(&format!("{}ping", prefix)) || content.starts_with(&format!("{}ping", alt_prefix)) {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                println!("Error sending message: {:?}", why);
            }
        } else if content.starts_with(&format!("{}search ", prefix)) || content.starts_with(&format!("{}search ", alt_prefix)) {
            let query = if content.starts_with(&format!("{}search ", prefix)) {
                content.trim_start_matches(&format!("{}search ", prefix)).trim()
            } else {
                content.trim_start_matches(&format!("{}search ", alt_prefix)).trim()
            };

            if query.is_empty() {
                msg.channel_id.say(&ctx.http, "Please enter a search query.").await.ok();
                return;
            }
             match env::var("TMDB_API_KEY") {
                Ok(tmdb_api_key) => {
                    if tmdb_api_key.is_empty() {
                        msg.channel_id.say(&ctx.http, "TMDB API key is configured but empty. Please contact the bot administrator.").await.ok();
                        return;
                    }
                    match search_tmdb(&tmdb_api_key, query).await {
                        Ok(results) => {
                            if results.is_empty() {
                                msg.channel_id.say(&ctx.http, "No results found for your query.").await.ok();
                                return;
                            }
                            let mut response_message = String::from("Search Results (Top 5):\n");
                            for (i, item) in results.iter().take(5).enumerate() {
                                let title = item.title.as_ref().or(item.name.as_ref()).unwrap_or(&"N/A".to_string());
                                let year = item.release_date.as_ref().or(item.first_air_date.as_ref())
                                    .and_then(|date_str| date_str.split('-').next())
                                    .unwrap_or("N/A");
                                let media_type_display = if item.media_type == "movie" { "Movie" } else if item.media_type == "tv" { "TV Show" } else { &item.media_type };
                                response_message.push_str(&format!("{}. {}: {} ({})\n", i + 1, media_type_display, title, year));
                            }
                            msg.channel_id.say(&ctx.http, &response_message).await.ok();
                        }
                        Err(err) => {
                            println!("Error searching TMDB: {:?}", err);
                            msg.channel_id.say(&ctx.http, "Error searching TMDB. Please try again later.").await.ok();
                        }
                    }
                }
                Err(_) => {
                    msg.channel_id.say(&ctx.http, "TMDB API key not configured. Please contact the bot administrator.").await.ok();
                }
            }

        } else if content.starts_with(&format!("{}imdb ", prefix)) || content.starts_with(&format!("{}imdb ", alt_prefix)) {
            let imdb_id = if content.starts_with(&format!("{}imdb ", prefix)) {
                content.trim_start_matches(&format!("{}imdb ", prefix)).trim()
            } else {
                content.trim_start_matches(&format!("{}imdb ", alt_prefix)).trim()
            };
            if !imdb_id.starts_with("tt") || imdb_id.len() < 3 { 
                msg.channel_id.say(&ctx.http, "Invalid IMDB ID format. It should start with 'tt'.").await.ok();
                return;
            }
             match env::var("TMDB_API_KEY") {
                Ok(tmdb_api_key) => {
                     if tmdb_api_key.is_empty() {
                        msg.channel_id.say(&ctx.http, "TMDB API key is configured but empty. Please contact the bot administrator.").await.ok();
                        return;
                    }
                    match find_by_imdb_id(&tmdb_api_key, imdb_id).await {
                        Ok(Some(item)) => {
                            let title = item.title.as_ref().or(item.name.as_ref()).unwrap_or(&"N/A".to_string());
                            let year = item.release_date.as_ref().or(item.first_air_date.as_ref())
                                .and_then(|date_str| date_str.split('-').next()).unwrap_or("N/A");
                            let media_type_display = if item.media_type == "movie" { "Movie" } else if item.media_type == "tv" { "TV Show" } else { &item.media_type };
                            let overview = item.overview.as_deref().unwrap_or("No overview available.");
                            let response_message = format!("**{}**: {} ({})\n**Overview**: {}", media_type_display, title, year, overview);
                            msg.channel_id.say(&ctx.http, &response_message).await.ok();

                            let tmdb_id_str = item.id.to_string();
                            let season_opt = if item.media_type == "tv" { Some(1) } else { None }; // Default S1E1 for test
                            let episode_opt = if item.media_type == "tv" { Some(1) } else { None }; // Default S1E1 for test
                            match get_vidsrc_streaming_url(&tmdb_id_str, &item.media_type, season_opt, episode_opt).await {
                                Ok(Some(vidsrc_url)) => {
                                    println!("[IMDB Command Test] Found VidSrc Player URL: {}", vidsrc_url);
                                     match resolve_player_url(&vidsrc_url).await {
                                        Ok(Some(direct_url)) => println!("[IMDB Command Test] Resolved direct media URL: {}", direct_url),
                                        Ok(None) => println!("[IMDB Command Test] Could not resolve direct media URL from: {}", vidsrc_url),
                                        Err(e) => println!("[IMDB Command Test] Error resolving player URL {}: {:?}", vidsrc_url, e),
                                    }
                                }
                                Ok(None) => println!("[IMDB Command Test] No VidSrc Player URL found for TMDB ID: {}", tmdb_id_str),
                                Err(e) => println!("[IMDB Command Test] Error getting VidSrc Player URL for TMDB ID {}: {:?}", tmdb_id_str, e),
                            }
                        }
                        Ok(None) => { msg.channel_id.say(&ctx.http, "No media found for that IMDB ID.").await.ok(); }
                        Err(err) => {
                            println!("Error finding by IMDB ID: {:?}", err);
                            msg.channel_id.say(&ctx.http, "Error fetching details from TMDB. Please try again later.").await.ok();
                        }
                    }
                }
                Err(_) => { msg.channel_id.say(&ctx.http, "TMDB API key not configured. Please contact the bot administrator.").await.ok(); }
            }
        } else if content.starts_with(&format!("{}join", prefix)) || content.starts_with(&format!("{}join", alt_prefix)) {
            let guild_id = match msg.guild_id {
                Some(id) => id,
                None => { msg.channel_id.say(&ctx.http, "This command can only be used in a server.").await.ok(); return; }
            };
            let channel_id = msg.guild(&ctx.cache).and_then(|guild| guild.voice_states.get(&msg.author.id).cloned()).and_then(|voice_state| voice_state.channel_id);
            let connect_to = match channel_id {
                Some(id) => id,
                None => { msg.channel_id.say(&ctx.http, "You need to be in a voice channel to use this command.").await.ok(); return; }
            };
            let manager = songbird::get(&ctx).await.expect("Songbird Voice client placed in context.").clone();
            if let Ok(call_lock) = manager.join(guild_id, connect_to).await {
                let mut call = call_lock.lock().await;
                call.deafen(true).await.ok();
                msg.channel_id.say(&ctx.http, &format!("Joined voice channel: <#{}>", connect_to.0)).await.ok();
            } else {
                msg.channel_id.say(&ctx.http, "Failed to join the voice channel.").await.ok();
            }
        } else if content.starts_with(&format!("{}play ", prefix)) || content.starts_with(&format!("{}play ", alt_prefix)) {
            let imdb_id_arg = if content.starts_with(&format!("{}play ", prefix)) { content.trim_start_matches(&format!("{}play ", prefix)).trim() } else { content.trim_start_matches(&format!("{}play ", alt_prefix)).trim() };
            if !imdb_id_arg.starts_with("tt") || imdb_id_arg.len() < 3 {
                msg.channel_id.say(&ctx.http, "Invalid IMDB ID format. It should start with 'tt'.").await.ok(); return;
            }
            let guild_id = match msg.guild_id {
                Some(id) => id,
                None => { msg.channel_id.say(&ctx.http, "This command can only be used in a server.").await.ok(); return; }
            };
            let manager = songbird::get(&ctx).await.expect("Songbird Voice client placed in context.").clone();
            if manager.get(guild_id).is_none() {
                 let user_channel_id = msg.guild(&ctx.cache).and_then(|guild| guild.voice_states.get(&msg.author.id).cloned()).and_then(|voice_state| voice_state.channel_id);
                match user_channel_id {
                    Some(channel_id_to_join) => {
                         if manager.join(guild_id, channel_id_to_join).await.is_err() {
                            msg.channel_id.say(&ctx.http, "Failed to join your voice channel.").await.ok(); return;
                        }
                        if let Some(handler_lock) = manager.get(guild_id) {
                            handler_lock.lock().await.deafen(true).await.ok();
                        }
                        msg.channel_id.say(&ctx.http, &format!("Joined your voice channel: <#{}>", channel_id_to_join.0)).await.ok();
                    },
                    None => { msg.channel_id.say(&ctx.http, "You are not in a voice channel. Please join one or use `m!join` first.").await.ok(); return; }
                }
            }
            let tmdb_api_key = match env::var("TMDB_API_KEY") {
                Ok(key) if !key.is_empty() => key,
                _ => { msg.channel_id.say(&ctx.http, "TMDB API key not configured or empty.").await.ok(); return; }
            };
            
            // Find media by IMDB ID
            let initial_media_item = match find_by_imdb_id(&tmdb_api_key, imdb_id_arg).await {
                Ok(Some(item)) => item,
                Ok(None) => { msg.channel_id.say(&ctx.http, "No media found for that IMDB ID.").await.ok(); return; }
                Err(e) => { println!("Error finding by IMDB ID for play: {:?}", e); msg.channel_id.say(&ctx.http, "Error fetching media details.").await.ok(); return; }
            };

            let tmdb_id_str = initial_media_item.id.to_string();
            let mut season_choice: Option<u32> = None;
            let mut episode_choice: Option<u32> = None;
            let show_name_for_title = initial_media_item.name.clone().unwrap_or_else(|| "Unknown TV Show".to_string());

            if initial_media_item.media_type == "tv" {
                // TV Show: Interactive season/episode selection
                let tv_details = match get_tmdb_tv_details(&tmdb_api_key, &tmdb_id_str).await {
                    Ok(details) => details,
                    Err(e) => {
                        println!("Error getting TMDB TV details: {:?}", e);
                        msg.channel_id.say(&ctx.http, "Could not fetch TV show details from TMDB.").await.ok();
                        return;
                    }
                };

                if tv_details.seasons.is_empty() || tv_details.number_of_seasons == 0 {
                    msg.channel_id.say(&ctx.http, format!("TV Show: {} has no seasons listed on TMDB.", tv_details.name)).await.ok();
                    return;
                }

                let season_prompt = format!(
                    "Found TV Show: {}. It has {} seasons.\nAvailable seasons: {}.\nPlease reply with the season number (e.g., '1') you want to watch.",
                    tv_details.name,
                    tv_details.number_of_seasons,
                    tv_details.seasons.iter().map(|s| s.season_number.to_string()).collect::<Vec<_>>().join(", ")
                );
                msg.channel_id.say(&ctx.http, &season_prompt).await.ok();

                if let Some(reply) = msg.channel_id.await_reply(&ctx).timeout(Duration::from_secs(30)).await {
                    match reply.content.parse::<u32>() {
                        Ok(s_num) if tv_details.seasons.iter().any(|s| s.season_number == s_num) => {
                            season_choice = Some(s_num);
                        }
                        _ => {
                            reply.channel_id.say(&ctx.http, "Invalid season number or selection timed out.").await.ok();
                            return;
                        }
                    }
                } else {
                    msg.channel_id.say(&ctx.http, "No season selected in time.").await.ok();
                    return;
                }
                
                // Fetch and display episodes for chosen season
                if let Some(s_num) = season_choice {
                    let season_details = match get_tmdb_season_details(&tmdb_api_key, &tmdb_id_str, s_num).await {
                        Ok(details) => details,
                        Err(e) => {
                            println!("Error getting TMDB season details: {:?}", e);
                            msg.channel_id.say(&ctx.http, "Could not fetch season episodes from TMDB.").await.ok();
                            return;
                        }
                    };
                    if season_details.episodes.is_empty() {
                         msg.channel_id.say(&ctx.http, format!("Season {} of {} has no episodes listed on TMDB.", s_num, tv_details.name)).await.ok();
                         return;
                    }

                    let mut episode_list_msg = format!("Season {}. Episodes (up to 10 shown):\n", s_num);
                    for ep in season_details.episodes.iter().take(10) {
                        episode_list_msg.push_str(&format!("{}. {}\n", ep.episode_number, ep.name));
                    }
                    if season_details.episodes.len() > 10 {
                        episode_list_msg.push_str("...\n");
                    }
                    episode_list_msg.push_str(&format!("Please reply with the episode number (1-{}).", season_details.episodes.last().map_or(0, |ep| ep.episode_number)));
                    
                    // Use the channel_id from the reply for the next await_reply, or original msg if reply was None (though we return if reply is None)
                    let reply_channel_id = msg.channel_id; // Simplified, as we return if first reply is None.
                    reply_channel_id.say(&ctx.http, &episode_list_msg).await.ok();

                    if let Some(ep_reply) = reply_channel_id.await_reply(&ctx).timeout(Duration::from_secs(30)).await {
                        match ep_reply.content.parse::<u32>() {
                            Ok(ep_num) if season_details.episodes.iter().any(|ep| ep.episode_number == ep_num) => {
                                episode_choice = Some(ep_num);
                            }
                            _ => {
                                ep_reply.channel_id.say(&ctx.http, "Invalid episode number or selection timed out.").await.ok();
                                return;
                            }
                        }
                    } else {
                        reply_channel_id.say(&ctx.http, "No episode selected in time.").await.ok();
                        return;
                    }
                }

            } else if initial_media_item.media_type == "movie" {
                // Movie: No season/episode selection needed
            } else {
                 msg.channel_id.say(&ctx.http, format!("Unsupported media type: {}", initial_media_item.media_type)).await.ok();
                return;
            }
            
            // Proceed with playback using tmdb_id_str, initial_media_item.media_type, season_choice, episode_choice
            let final_season = season_choice.map(|s| s as usize);
            let final_episode = episode_choice.map(|e| e as usize);

            let player_page_url = match get_vidsrc_streaming_url(&tmdb_id_str, &initial_media_item.media_type, final_season, final_episode).await {
                Ok(Some(url)) => url,
                Ok(None) => { msg.channel_id.say(&ctx.http, "Could not find a VidSrc streaming page for the selected media/episode.").await.ok(); return; }
                Err(e) => { println!("Error getting VidSrc URL for play: {:?}", e); msg.channel_id.say(&ctx.http, "Error getting streaming page URL.").await.ok(); return; }
            };
            let direct_media_url = match resolve_player_url(&player_page_url).await {
                Ok(Some(url)) => url,
                Ok(None) => { msg.channel_id.say(&ctx.http, "Could not resolve a direct playable media URL.").await.ok(); return; }
                Err(e) => { println!("Error resolving player URL for play: {:?}", e); msg.channel_id.say(&ctx.http, "Error resolving direct media URL.").await.ok(); return; }
            };

            if let Some(handler_lock) = manager.get(guild_id) {
                let mut call = handler_lock.lock().await;
                call.stop(); 
                let source = match HttpRequest::new(HTTP_CLIENT.clone(), direct_media_url.clone()).await {
                    Ok(s) => s,
                    Err(e) => { println!("Error creating HttpRequest source: {:?}", e); msg.channel_id.say(&ctx.http, format!("Error creating stream source: {}", e)).await.ok(); return; }
                };
                let track_handle = call.play_source(Input::from(source));
                
                let full_title = if initial_media_item.media_type == "tv" {
                    format!("{} S{}E{} (IMDB: {})", show_name_for_title, season_choice.unwrap_or(0), episode_choice.unwrap_or(0), imdb_id_arg)
                } else {
                    format!("{} (IMDB: {})", initial_media_item.title.as_ref().unwrap_or(&"Unknown Movie".to_string()), imdb_id_arg)
                };
                
                *self.current_track_title.lock().await = Some(full_title.clone());
                if let Err(e) = track_handle.add_event(Event::Track(TrackEvent::End), TrackEndNotifier { current_track_title: self.current_track_title.clone() }) {
                    println!("Failed to add track end event handler: {:?}", e);
                }
                msg.channel_id.say(&ctx.http, &format!("Now playing: {}", full_title)).await.ok();
            } else {
                msg.channel_id.say(&ctx.http, "Not in a voice channel. Use `m!join` first.").await.ok();
            }

        } else if content.starts_with(&format!("{}pause", prefix)) || content.starts_with(&format!("{}pause", alt_prefix)) {
            let guild_id = match msg.guild_id { Some(id) => id, None => { msg.channel_id.say(&ctx.http, "Command only for servers.").await.ok(); return; } };
            let manager = songbird::get(&ctx).await.expect("Songbird Voice client placed in context.").clone();
            if let Some(handler_lock) = manager.get(guild_id) {
                let call = handler_lock.lock().await;
                if call.queue().pause().is_ok() { msg.channel_id.say(&ctx.http, "Playback paused.").await.ok(); } 
                else { msg.channel_id.say(&ctx.http, "Failed to pause playback. Nothing playing?").await.ok(); }
            } else { msg.channel_id.say(&ctx.http, "Not in a voice channel.").await.ok(); }
        } else if content.starts_with(&format!("{}resume", prefix)) || content.starts_with(&format!("{}resume", alt_prefix)) {
            let guild_id = match msg.guild_id { Some(id) => id, None => { msg.channel_id.say(&ctx.http, "Command only for servers.").await.ok(); return; } };
            let manager = songbird::get(&ctx).await.expect("Songbird Voice client placed in context.").clone();
            if let Some(handler_lock) = manager.get(guild_id) {
                let call = handler_lock.lock().await;
                if call.queue().resume().is_ok() { msg.channel_id.say(&ctx.http, "Playback resumed.").await.ok(); }
                else { msg.channel_id.say(&ctx.http, "Failed to resume playback.").await.ok(); }
            } else { msg.channel_id.say(&ctx.http, "Not in a voice channel.").await.ok(); }
        } else if content.starts_with(&format!("{}stop", prefix)) || content.starts_with(&format!("{}stop", alt_prefix)) {
            let guild_id = match msg.guild_id { Some(id) => id, None => { msg.channel_id.say(&ctx.http, "Command only for servers.").await.ok(); return; } };
            let manager = songbird::get(&ctx).await.expect("Songbird Voice client placed in context.").clone();
            if let Some(handler_lock) = manager.get(guild_id) {
                let mut call = handler_lock.lock().await;
                call.stop();
                *self.current_track_title.lock().await = None; 
                let leave_after_stop = true; 
                if leave_after_stop {
                    if manager.leave(guild_id).await.is_ok() {
                        msg.channel_id.say(&ctx.http, "Playback stopped and left voice channel.").await.ok();
                    } else {
                        msg.channel_id.say(&ctx.http, "Playback stopped. Failed to leave voice channel.").await.ok();
                    }
                } else {
                    msg.channel_id.say(&ctx.http, "Playback stopped.").await.ok();
                }
            } else { msg.channel_id.say(&ctx.http, "Not in a voice channel.").await.ok(); }
        } else if content.starts_with(&format!("{}volume ", prefix)) || content.starts_with(&format!("{}volume ", alt_prefix)) {
            let guild_id = match msg.guild_id { Some(id) => id, None => { msg.channel_id.say(&ctx.http, "Command only for servers.").await.ok(); return; } };
            let manager = songbird::get(&ctx).await.expect("Songbird Voice client placed in context.").clone();
            if let Some(handler_lock) = manager.get(guild_id) {
                let arg = if content.starts_with(&format!("{}volume ", prefix)) { content.trim_start_matches(&format!("{}volume ", prefix)).trim() } else { content.trim_start_matches(&format!("{}volume ", alt_prefix)).trim() };
                match arg.parse::<f32>() {
                    Ok(volume_percent) => {
                        if (0.0..=100.0).contains(&volume_percent) {
                            let volume_float = volume_percent / 100.0;
                            let call = handler_lock.lock().await;
                            if let Some(track_handle) = call.queue().current() {
                                if track_handle.set_volume(volume_float).is_ok() {
                                    msg.channel_id.say(&ctx.http, &format!("Volume set to {:.0}%.", volume_percent)).await.ok();
                                } else {
                                    msg.channel_id.say(&ctx.http, "Failed to set volume.").await.ok();
                                }
                            } else { msg.channel_id.say(&ctx.http, "Nothing is playing.").await.ok(); }
                        } else { msg.channel_id.say(&ctx.http, "Volume must be between 0 and 100.").await.ok(); }
                    }
                    Err(_) => { msg.channel_id.say(&ctx.http, "Invalid volume percentage. Please use a number (0-100).").await.ok(); }
                }
            } else { msg.channel_id.say(&ctx.http, "Not in a voice channel.").await.ok(); }
        } else if content.starts_with(&format!("{}playing", prefix)) || content.starts_with(&format!("{}playing", alt_prefix)) {
             let title_guard = self.current_track_title.lock().await;
             if let Some(title) = &*title_guard {
                 msg.channel_id.say(&ctx.http, &format!("Currently playing: {}", title)).await.ok();
             } else {
                 msg.channel_id.say(&ctx.http, "Nothing is currently playing.").await.ok();
             }
        } else if content.starts_with(&format!("{}skip ", prefix)) || content.starts_with(&format!("{}skip ", alt_prefix)) {
            let guild_id = match msg.guild_id { Some(id) => id, None => { msg.channel_id.say(&ctx.http, "Command only for servers.").await.ok(); return; } };
            let manager = songbird::get(&ctx).await.expect("Songbird Voice client placed in context.").clone();
            if let Some(handler_lock) = manager.get(guild_id) {
                let arg = if content.starts_with(&format!("{}skip ", prefix)) { content.trim_start_matches(&format!("{}skip ", prefix)).trim() } else { content.trim_start_matches(&format!("{}skip ", alt_prefix)).trim() };
                match arg.parse::<u64>() {
                    Ok(seconds) => {
                        let call = handler_lock.lock().await;
                        if let Some(track_handle) = call.queue().current() {
                            let duration = Duration::from_secs(seconds);
                            match track_handle.seek_time(duration) {
                                Ok(_) => msg.channel_id.say(&ctx.http, &format!("Skipped to {} seconds.", seconds)).await.ok(),
                                Err(e) => {
                                    println!("Error seeking track: {:?}", e);
                                    msg.channel_id.say(&ctx.http, "Failed to skip. The stream might not be seekable.").await.ok();
                                }
                            }
                        } else { msg.channel_id.say(&ctx.http, "Nothing is playing.").await.ok(); }
                    }
                    Err(_) => { msg.channel_id.say(&ctx.http, "Invalid time. Please provide seconds (e.g., `m!skip 60`).").await.ok(); }
                }
            } else { msg.channel_id.say(&ctx.http, "Not in a voice channel.").await.ok(); }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a DISCORD_TOKEN in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new()) 
        .register_songbird()
        .await
        .expect("Err creating client");

    tokio::spawn(async move {
        if let Err(why) = client.start().await {
            println!("Client error: {:?}", why);
        }
    });
    
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    println!("Ctrl-C received, shutting down.");
}
