use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{env, fs};

use serenity::async_trait;
use serenity::framework::standard::CommandResult;

use serenity::model::id::{ChannelId, GuildId};

use serenity::framework::standard::macros::{command, group};
use serenity::framework::StandardFramework;
use serenity::model::channel::Message;
use serenity::prelude::*;
use twitter_v2::authorization::BearerToken;
use twitter_v2::id::NumericId;
use twitter_v2::{Error, TwitterApi};

#[group]
#[commands(help)]
struct General;

struct Handler {
    is_loop_running: AtomicBool,
    mappings: Vec<(NumericId, ChannelId)>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        let ctx = Arc::new(ctx);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            let ctx1 = Arc::clone(&ctx);
            let mappings = self.mappings.clone();
            tokio::spawn(async move {
                let cache_dir = Path::new("cache");
                fs::create_dir_all(cache_dir).unwrap();
                let likes_file = cache_dir.join("likes.json");

                let mut post_cache: HashSet<u64> = if likes_file.exists() {
                    serde_json::from_str(fs::read_to_string(&likes_file).unwrap().as_str()).unwrap()
                } else {
                    HashSet::new()
                };

                loop {
                    if let Ok(true) =
                        check_twitter(Arc::clone(&ctx1), &mappings, &mut post_cache).await
                    {
                        fs::write(&likes_file, serde_json::to_string(&post_cache).unwrap())
                            .unwrap();
                    }
                    tokio::time::sleep(Duration::from_secs(180)).await;
                }
            });

            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

async fn check_twitter(
    ctx: Arc<Context>,
    cfg: &Vec<(NumericId, ChannelId)>,
    post_cache: &mut HashSet<u64>,
) -> Result<bool, Error> {
    let auth = BearerToken::new(env::var("TWITTER_TOKEN").unwrap());
    let api = TwitterApi::new(auth);
    let mut mutated = false;

    for (user_id, channel) in cfg {
        let likes = api
            .get_user_liked_tweets(*user_id)
            .max_results(20)
            .send()
            .await?
            .into_data();

        if let Some(mut likes_vec) = likes {
            likes_vec.sort_by_key(|l| Reverse(l.id));

            for like in likes_vec {
                let post_nr: u64 = like.id.into();

                if post_cache.contains(&post_nr) {
                    continue;
                }

                channel
                    .send_message(&ctx, |m| {
                        m.content(format!("https://twitter.com/twitter/status/{}", post_nr))
                            .allowed_mentions(|m| m.empty_parse())
                    })
                    .await
                    .ok();

                post_cache.insert(post_nr);
                mutated = true;
            }
        }
    }

    Ok(mutated)
}

#[tokio::main]
async fn main() {
    let twitter_user = env::var("TWITTER_USER")
        .expect("twitter user")
        .parse::<u64>()
        .expect("number");

    let discord_channel = env::var("DISCORD_CHANNEL")
        .expect("discord channel")
        .parse::<u64>()
        .expect("number");

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("ß"))
        .group(&GENERAL_GROUP);

    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler {
            is_loop_running: AtomicBool::new(false),
            mappings: vec![(NumericId::new(twitter_user), ChannelId(discord_channel))],
        })
        .framework(framework)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "I have no commands! :c").await?;

    Ok(())
}
