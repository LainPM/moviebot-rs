use std::borrow::Cow;

use crate::{client::client::{Context, Error}, server::{classes::{fasel::Fasel, netflix::{Netflix, NetflixSearcher, ShowResult}}, functions::quit_browser}};
use image::{DynamicImage, ImageBuffer, Rgba};
use poise::serenity_prelude::CreateAttachment;
use serenity::all::{CreateEmbed, EditAttachments, EditMessage};

use crate::server::classes::fasel::FaselSearcher;
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "This is an example bot made to showcase features of my custom Discord bot framework",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}


#[poise::command(prefix_command, slash_command)]
pub async fn vote(
    ctx: Context<'_>,
    #[description = "What to vote for"] choice: String,
) -> Result<(), Error> {
    let num_votes = {
        let mut hash_map = ctx.data().votes.lock().unwrap();
        let num_votes = hash_map.entry(choice.clone()).or_default();
        *num_votes += 1;
        *num_votes
    };

    let response = format!("Successfully voted for {choice}. {choice} now has {num_votes} votes!");
    ctx.say(response).await?;
    Ok(())
}
async fn fasel_autocomplete<'a>(
    _ctx: Context<'a>,
    _partial: &str,
) -> Vec<String> {
    vec!["Netflix".to_string(), "Fasel".to_string()]
}
#[poise::command(slash_command)]
pub async fn watch(
    ctx: Context<'_>,
    #[description = "Choose a movie"] movie: String,
    #[autocomplete = "fasel_autocomplete"] streamingservice: String,
) -> Result<(), Error> {
    let loading_message = ctx.say("Loading...").await?;
    let mut message = match loading_message.message().await? {
        Cow::Owned(msg) => msg,
        Cow::Borrowed(msg) => msg.clone(),
    };

    let movie_data = fetch_movies(&movie, &streamingservice).await?;
    if movie_data.is_empty() {
        ctx.say("Couldn't find movie.").await?;
        return Ok(());
    }

    let buffer = create_movie_collage(&movie_data).await?;
    let new_attachment = CreateAttachment::bytes(buffer, "movies.png".to_string());
    let attachment = EditAttachments::new().add(new_attachment);

    let embed = CreateEmbed::default()
        .title(format!("Here are movies for '{}'", movie))
        .description(format!("Select a movie to watch on {}", streamingservice))
        .image("attachment://movies.png")
        .to_owned();

    let options: Vec<poise::serenity_prelude::CreateSelectMenuOption> = movie_data
        .iter()
        .map(|(_, _, title, _)| poise::serenity_prelude::CreateSelectMenuOption::new(title.clone(), title.clone()))
        .collect();

    let select_menu = poise::serenity_prelude::CreateSelectMenu::new(
        format!("movieSelect:{}", streamingservice),
        serenity::all::CreateSelectMenuKind::String { options },
    )
    .placeholder("Select a movie");

    let action_row = poise::serenity_prelude::CreateActionRow::SelectMenu(select_menu).to_owned();

    let reply = EditMessage::new()
        .content("")
        .embed(embed.clone())
        .components(vec![action_row])
        .attachments(attachment);

    message.edit(ctx.http(), reply).await?;
    Ok(())
}

async fn fetch_movies(movie: &str, streamingservice: &str) -> Result<Vec<(String, String, String, ShowResult)>, Error> {
    match streamingservice.to_lowercase().as_str() {
        "fasel" => FaselSearcher::_search(movie).await,
        "netflix" => NetflixSearcher::search(movie).await,
        _ => Err("Streaming service not supported".into()),
    }
}

async fn create_movie_collage(movie_data: &Vec<(String, String, String, ShowResult)>) -> Result<Vec<u8>, Error> {
    let movie_count = movie_data.len() as u32;
    let max_columns = (movie_count as f64).sqrt().ceil() as u32;
    let max_rows = (movie_count as f64 / max_columns as f64).ceil() as u32;

    let image_width: u32 = 250;
    let image_height: u32 = 350;
    let padding: u32 = 20;
    let frame_thickness: u32 = 10;
    let background_color = Rgba([30, 30, 30, 255]);

    let canvas_width = max_columns * (image_width + padding + frame_thickness * 2);
    let canvas_height = max_rows * (image_height + padding + frame_thickness * 2);
    
    let mut canvas = ImageBuffer::from_pixel(canvas_width, canvas_height, background_color);

    for (index, (_, image_url, _, _)) in movie_data.iter().enumerate() {
        let x_pos = (index as u32 % max_columns) * (image_width + padding + frame_thickness * 2);
        let y_pos = (index as u32 / max_columns) * (image_height + padding + frame_thickness * 2);

        match reqwest::get(image_url).await {
            Ok(response) => {
                if let Ok(bytes) = response.bytes().await {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let resized_img = img.resize(image_width, image_height, image::imageops::FilterType::Lanczos3);
                        image::imageops::overlay(&mut canvas, &resized_img, (x_pos + frame_thickness) as i64, (y_pos + frame_thickness) as i64);
                    }
                }
            }
            Err(_) => {
                let placeholder_img = DynamicImage::new_rgb8(image_width, image_height);
                image::imageops::overlay(&mut canvas, &placeholder_img, (x_pos + frame_thickness) as i64, (y_pos + frame_thickness) as i64);
            }
        }
    }

    let mut buffer = Vec::new();
    canvas.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
        .expect("Failed to write image");

    Ok(buffer)
}


#[poise::command(slash_command)]
pub async fn stop(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let _ = quit_browser().await;
    ctx.reply("Stopped The Player").await?;
    Ok(())
}
#[poise::command(slash_command)]
pub async fn skip_to(
    ctx: Context<'_>
) -> Result<(), Error> {
    Netflix::skip_to_specific_timeline("0.20").await?;
    ctx.reply("Skipped").await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn pause(
    _ctx: Context<'_>,
) -> Result<(), Error> {
    Fasel::pause().await?;
    Ok(())
}


#[poise::command(prefix_command, track_edits, aliases("votes"), slash_command)]
pub async fn getvotes(
    ctx: Context<'_>,
    #[description = "Choice to retrieve votes for"] choice: Option<String>,
) -> Result<(), Error> {
    let response = if let Some(choice) = choice {
        let num_votes = *ctx.data().votes.lock().unwrap().get(&choice).unwrap_or(&0);
        match num_votes {
            0 => format!("Nobody has voted for {} yet", choice),
            _ => format!("{} people have voted for {}", num_votes, choice),
        }
    } else {
        let mut response = String::new();
        let hash_map = ctx.data().votes.lock().unwrap();
        for (choice, num_votes) in hash_map.iter() {
            response += &format!("{}: {} votes\n", choice, num_votes);
        }

        if response.is_empty() {
            response += "Nobody has voted for anything yet :(";
        }

        response
    };

    ctx.say(response).await?;
    Ok(())
}
