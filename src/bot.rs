use futures::stream::StreamExt;
use std::{env, error::Error, sync::Arc, sync::Mutex};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, Event};
use twilight_http::Client as HttpClient;

use crate::album::Album;

mod command;

pub fn set_sigint_handler(alb: Arc<Mutex<Album>>) -> Result<(), ctrlc::Error> {
    let res = ctrlc::set_handler(move || {
        eprintln!("stoping...");
        match alb.lock() {
            Ok(alb) => match alb.save() {
                Ok(_) => {
                    eprintln!("saved album sucessfully");
                }
                Err(err) => {
                    eprintln!("failed to save album.");
                    eprintln!("{}", err);
                }
            },
            Err(err) => {
                eprintln!("failed to lock album in order to save it.");
                eprintln!("{}", err);
            }
        }
        std::process::exit(0);
    });
    return res;
}

pub async fn start(alb: crate::album::Album) -> anyhow::Result<()> {
    let token = env::var("DISCORD_TOKEN")?;
    let alb = Arc::new(Mutex::new(alb));

    if let Err(err) = set_sigint_handler(Arc::clone(&alb)) {
        eprintln!("failed to set a sigint handler, album will not be save when quitting.");
        eprintln!("{}", err);
    }

    // A cluster is a manager for multiple shards that by default
    // creates as many shards as Discord recommends.
    let (cluster, mut events) = Cluster::new(
        token.to_owned(),
        twilight_gateway::Intents::GUILD_MESSAGES | twilight_gateway::Intents::MESSAGE_CONTENT,
    )
    .await?;
    let cluster = Arc::new(cluster);

    // Start up the cluster.
    let cluster_spawn = Arc::clone(&cluster);

    // Start all shards in the cluster in the background.
    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    // HTTP is separate from the gateway, so create a new client.
    let client = Arc::new(HttpClient::new(token));

    // Since we only care about new messages, make the cache only
    // cache new messages.
    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    // Process each event as they come in.
    while let Some((shard_id, event)) = events.next().await {
        // Update the cache with the event.
        cache.update(&event);

        tokio::spawn(handle_event(
            shard_id,
            event,
            Arc::clone(&client),
            Arc::clone(&alb),
        ));
    }

    Ok(())
}

async fn slash_command_trial(client: &HttpClient) -> Result<(), anyhow::Error> {
    let application_id = {
        let response = client.current_user_application().exec().await?;
        response.model().await?.id
    };

    let interact_client = client.interaction(application_id);
    const ID: twilight_model::id::Id<twilight_model::id::marker::GuildMarker> =
        unsafe { twilight_model::id::Id::new_unchecked(416194652744450048) };

    let commands = client
        .interaction(application_id)
        .guild_commands(ID)
        .exec()
        .await?
        .models()
        .await?;
    println!("there are {} guild commands", commands.len());

    for cmd in commands {
        if let Some(cmd_id) = cmd.id {
            interact_client
                .delete_guild_command(ID, cmd_id)
                .exec()
                .await
                .unwrap();
        }
    }

    Ok(())
}

async fn handle_event(
    shard_id: u64,
    event: Event,
    client: Arc<HttpClient>,
    album: Arc<Mutex<crate::album::Album>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) if msg.content.contains("patate") => {
            client
                .create_message(msg.channel_id)
                .content("Pong!")?
                .exec()
                .await?;
        }
        Event::MessageCreate(msg) if msg.content.starts_with("!xadd") => {
            command::picture_add(msg, &album, &client).await?;
        }
        Event::MessageCreate(msg) if msg.content.starts_with("!delete_last") => {
            command::delete_last(msg, &album, &client).await?;
        }
        Event::MessageCreate(msg) if msg.content.starts_with("!delete_pic") => {
            command::delete_picture(msg, &album, &client).await?;
        }
        Event::MessageCreate(msg) if msg.content.len() > 1 => {
            command::picture_find_and_send(album, msg, client).await?;
        }
        Event::ShardConnected(_) => {
            println!("Connected on shard {shard_id}");
        }
        _ => {}
    }

    Ok(())
}
