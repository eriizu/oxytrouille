use futures::stream::StreamExt;
use std::{env, error::Error, ops::BitAnd, sync::Arc, sync::Mutex};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, Event};
use twilight_http::Client as HttpClient;

use crate::album::Album;

struct BotState {
    album: Arc<Mutex<crate::album::Album>>,
    admin_roles: Vec<id::Id<id::marker::RoleMarker>>,
}

impl BotState {
    fn new(
        alb: Arc<Mutex<crate::album::Album>>,
        admin_roles: Vec<id::Id<id::marker::RoleMarker>>,
    ) -> Self {
        Self {
            album: alb,
            admin_roles,
        }
    }
}

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
        twilight_gateway::Intents::GUILD_MESSAGES
            | twilight_gateway::Intents::MESSAGE_CONTENT
            | twilight_gateway::Intents::GUILD_MESSAGE_REACTIONS,
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

    let admin_roles = find_roles_admin(&client).await?;

    // Process each event as they come in.
    while let Some((shard_id, event)) = events.next().await {
        // Update the cache with the event.
        cache.update(&event);

        tokio::spawn(handle_event(
            shard_id,
            event,
            Arc::clone(&client),
            BotState::new(Arc::clone(&alb), Vec::clone(&admin_roles)),
        ));
    }

    Ok(())
}

async fn find_roles_admin(
    client: &HttpClient,
) -> Result<Vec<id::Id<id::marker::RoleMarker>>, anyhow::Error> {
    let mut out: Vec<id::Id<id::marker::RoleMarker>> = Vec::new();

    let roles = client.roles(GUILD_ID).exec().await?.model().await?;
    for role in roles {
        if role
            .permissions
            .contains(twilight_model::guild::Permissions::ADMINISTRATOR)
        {
            out.push(role.id);
        }
    }

    return Ok(out);
}

use twilight_model::id;

const GUILD_ID: id::Id<id::marker::GuildMarker> =
    unsafe { id::Id::new_unchecked(416194652744450048) };

const PRONOUN_MESSAGE_ID: id::Id<id::marker::MessageMarker> =
    unsafe { id::Id::new_unchecked(606807344759963688) };

async fn handle_event(
    shard_id: u64,
    event: Event,
    client: Arc<HttpClient>,
    state: BotState,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) if msg.content.contains("patate") => {
            client
                .create_message(msg.channel_id)
                .content("Pong!")?
                .exec()
                .await?;
        }
        Event::MessageCreate(msg) if msg.content.starts_with("!add") => {
            if admin_guard(&msg, &state, &client).await? {
                command::picture_add(msg, &state.album, &client).await?;
            }
        }
        Event::MessageCreate(msg) if msg.content.starts_with("!delete_last") => {
            if admin_guard(&msg, &state, &client).await? {
                command::delete_last(msg, &state.album, &client).await?;
            }
        }
        Event::MessageCreate(msg) if msg.content.starts_with("!delete_pic") => {
            if admin_guard(&msg, &state, &client).await? {
                command::delete_picture(msg, &state.album, &client).await?;
            }
        }
        Event::MessageCreate(msg) if msg.content.len() > 1 => {
            command::picture_find_and_send(state.album, msg, client).await?;
        }
        Event::ReactionAdd(reaction) => {
            if reaction.message_id == PRONOUN_MESSAGE_ID {
                if let twilight_model::channel::ReactionType::Unicode { name } = &reaction.emoji {
                    let role_id = role_from_emoji(name);
                    if let Some(role_id) = role_id {
                        client
                            .add_guild_member_role(GUILD_ID, reaction.user_id, role_id)
                            .exec()
                            .await
                            .unwrap();
                    }
                }
            }
        }
        Event::ReactionRemove(reaction) => {
            if reaction.message_id == PRONOUN_MESSAGE_ID {
                if let twilight_model::channel::ReactionType::Unicode { name } = &reaction.emoji {
                    let role_id = role_from_emoji(name);
                    if let Some(role_id) = role_id {
                        client
                            .remove_guild_member_role(GUILD_ID, reaction.user_id, role_id)
                            .exec()
                            .await
                            .unwrap();
                    }
                }
            }
        }
        Event::ShardConnected(_) => {
            println!("Connected on shard {shard_id}");
        }
        _ => {}
    }

    Ok(())
}

async fn admin_guard(
    msg: &Box<twilight_model::gateway::payload::incoming::MessageCreate>,
    state: &BotState,
    client: &Arc<HttpClient>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    Ok(if let Some(member) = &msg.member {
        let first_admin_role = member
            .roles
            .iter()
            .find(|item| state.admin_roles.contains(item));
        if let Some(_) = first_admin_role {
            true
        } else {
            client
                .create_message(msg.channel_id)
                .reply(msg.id)
                .content("Seul un·e admin peut faire ceci.")?
                .exec()
                .await?;
            false
        }
    } else {
        false
    })
}

fn role_from_emoji(name: &String) -> Option<id::Id<id::marker::RoleMarker>> {
    match name.as_str() {
        "🌻" => unsafe { Some(twilight_model::id::Id::new_unchecked(606807806938447872)) },
        "🌸" => unsafe { Some(twilight_model::id::Id::new_unchecked(606807957052588042)) },
        "🍀" => unsafe { Some(twilight_model::id::Id::new_unchecked(606808023108943872)) },
        "🌼" => unsafe { Some(twilight_model::id::Id::new_unchecked(606808071834173451)) },

        _ => None,
    }
}
