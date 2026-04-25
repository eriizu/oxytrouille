use std::{env, error::Error, sync::Arc, sync::Mutex};
use twilight_gateway::Event;
use twilight_http::Client as HttpClient;

use crate::album::Album;

struct BotState {
    album: Arc<Mutex<crate::album::Album>>,
    admin_roles: Vec<id::Id<id::marker::RoleMarker>>,
    pause_yell: Arc<Mutex<Option<std::time::Instant>>>,
}

impl BotState {
    fn new(
        alb: Arc<Mutex<crate::album::Album>>,
        admin_roles: Vec<id::Id<id::marker::RoleMarker>>,
        pause_yell_timer: Arc<Mutex<Option<std::time::Instant>>>,
    ) -> Self {
        Self {
            album: alb,
            admin_roles,
            pause_yell: pause_yell_timer,
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

    tracing_subscriber::fmt::init();

    if let Err(err) = set_sigint_handler(Arc::clone(&alb)) {
        tracing::error!(
            ?err,
            "failed to set a sigint handler, album will not be save when quitting."
        );
    }

    let intents = twilight_gateway::Intents::GUILD_MESSAGES
        | twilight_gateway::Intents::MESSAGE_CONTENT
        | twilight_gateway::Intents::GUILD_MESSAGE_REACTIONS
        | twilight_model::gateway::Intents::GUILD_MEMBERS;

    let mut shard =
        twilight_gateway::Shard::new(twilight_gateway::ShardId::ONE, token.clone(), intents);

    let client = Arc::new(HttpClient::new(token));

    let admin_roles = find_roles_admin(&client).await?;
    let pause_yell_timer = Arc::new(Mutex::new(None));

    let cache = twilight_cache_inmemory::InMemoryCache::new();

    tracing::info!("ready, starting loop");

    loop {
        let event = match shard.next_event().await {
            Ok(event) => event,
            Err(source) => {
                tracing::warn!(?source, "error receiving event");
                if source.is_fatal() {
                    break;
                }
                continue;
            }
        };

        cache.update(&event);

        tokio::spawn(handle_event(
            event,
            Arc::clone(&client),
            BotState::new(
                Arc::clone(&alb),
                Vec::clone(&admin_roles),
                Arc::clone(&pause_yell_timer),
            ),
        ));
    }
    return Ok(());
}

async fn find_roles_admin(
    client: &HttpClient,
) -> Result<Vec<id::Id<id::marker::RoleMarker>>, anyhow::Error> {
    let mut out: Vec<id::Id<id::marker::RoleMarker>> = Vec::new();

    let jp_roles = client.roles(JAPANESE_GUILD_ID).await?.model().await?;
    let st_roles = client.roles(STREAMING_GUILD_ID).await?.model().await?;

    let iter = jp_roles.iter().chain(st_roles.iter());
    for role in iter {
        if role
            .permissions
            .contains(twilight_model::guild::Permissions::ADMINISTRATOR)
        {
            out.push(role.id);
        }
    }

    return Ok(out);
}

use twilight_model::{gateway::payload::incoming::MessageCreate, id};

const JAPANESE_GUILD_ID: id::Id<id::marker::GuildMarker> =
    unsafe { id::Id::new_unchecked(416194652744450048) };

const STREAMING_GUILD_ID: id::Id<id::marker::GuildMarker> =
    unsafe { id::Id::new_unchecked(1102170640649293864) };

const PRONOUN_MESSAGE_ID: id::Id<id::marker::MessageMarker> =
    unsafe { id::Id::new_unchecked(606807344759963688) };

const PROTECTED_USER_ID: id::Id<id::marker::UserMarker> =
    unsafe { id::Id::new_unchecked(350629483042177025) };

const BAN_EMOJI_ID: id::Id<id::marker::EmojiMarker> =
    unsafe { id::Id::new_unchecked(519852990119673871) };

async fn handle_event(
    event: Event,
    client: Arc<HttpClient>,
    state: BotState,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    //let trace = tracing::span!(tracing::Level::INFO, "handle event");
    match event {
        Event::MessageCreate(msg) => handle_message(msg, client, state).await?,
        Event::ReactionAdd(reaction) => {
            if reaction.message_id == PRONOUN_MESSAGE_ID {
                if let twilight_model::channel::message::ReactionType::Unicode { name } =
                    &reaction.emoji
                {
                    let role_id = role_from_emoji(name);
                    if let Some(role_id) = role_id {
                        client
                            .add_guild_member_role(JAPANESE_GUILD_ID, reaction.user_id, role_id)
                            .await
                            .unwrap();
                    }
                }
            }
        }
        Event::ReactionRemove(reaction) => {
            if reaction.message_id == PRONOUN_MESSAGE_ID {
                if let twilight_model::channel::message::ReactionType::Unicode { name } =
                    &reaction.emoji
                {
                    let role_id = role_from_emoji(name);
                    if let Some(role_id) = role_id {
                        client
                            .remove_guild_member_role(JAPANESE_GUILD_ID, reaction.user_id, role_id)
                            .await
                            .unwrap();
                    }
                }
            }
        }
        // Event::MessageCreate(message) => {
        //     let author = &message.author.name;
        //     let content = &message.content;
        //     tracing::debug!(author, content, "no action for this message");
        //     //eprintln!("nothing to do with {:?}", message);
        // }
        _ => {}
    }

    Ok(())
}

async fn handle_message(
    msg: Box<MessageCreate>,
    client: Arc<HttpClient>,
    state: BotState,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if msg.author.bot {
        eprintln!("ignoring bot command from {}", msg.author.id);
    }
    if msg.content.contains("poru") {
        use twilight_http::request::channel::reaction;
        let emoji = reaction::RequestReactionType::Unicode { name: "🇵" };
        client
            .create_reaction(msg.channel_id, msg.id, &emoji)
            .await?;
        let emoji = reaction::RequestReactionType::Unicode { name: "🇴" };
        client
            .create_reaction(msg.channel_id, msg.id, &emoji)
            .await?;
        let emoji = reaction::RequestReactionType::Unicode { name: "🇷" };
        client
            .create_reaction(msg.channel_id, msg.id, &emoji)
            .await?;
        let emoji = reaction::RequestReactionType::Unicode { name: "🇺" };
        client
            .create_reaction(msg.channel_id, msg.id, &emoji)
            .await?;
    }
    if msg.content.starts_with("!reset_nick") {
        if admin_guard(&msg, &state, &client).await? {
            command::reset_nick(msg, &client).await?;
        }
    } else if msg.content.starts_with("!gouvernement") {
        client
            .create_message(msg.channel_id)
            .reply(msg.id)
            .content("https://clips.twitch.tv/FriendlyResilientSlothShazBotstix-HWxnFQWq6iPPsVZf")?
            .await?;
    } else if msg.content.starts_with("!add") {
        if admin_guard(&msg, &state, &client).await? {
            command::picture_add(msg, &state.album, &client).await?;
        }
    } else if msg.content.starts_with("!resume_yelling") {
        if admin_guard(&msg, &state, &client).await? {
            // state.pause_yell = Some(std::time::Instant::now());
            let text_to_reply = if let Ok(mut pause_yell) = state.pause_yell.lock() {
                *pause_yell = None;
                "分かりました。"
            } else {
                "J'arrive pas à reset mon timer."
            };
            client
                .create_message(msg.channel_id)
                .reply(msg.id)
                .content(text_to_reply)?
                .await?;
        }
    } else if msg.content.starts_with("!quit_yelling") {
        if admin_guard(&msg, &state, &client).await? {
            let text_to_reply = if let Ok(mut pause_yell) = state.pause_yell.lock() {
                *pause_yell = Some(std::time::Instant::now());
                "Ok je me calme pendant une heure ou deux."
            } else {
                "J'arrive pas à me canaliser..."
            };
            client
                .create_message(msg.channel_id)
                .reply(msg.id)
                .content(text_to_reply)?
                .await?;
        }
    } else if msg.content.starts_with("!delete_last") {
        if admin_guard(&msg, &state, &client).await? {
            command::delete_last(msg, &state.album, &client).await?;
        }
    } else if msg.content.starts_with("!delete_pic") {
        if admin_guard(&msg, &state, &client).await? {
            command::delete_picture(msg, &state.album, &client).await?;
        }
    } else if msg.content.starts_with("!aled") {
        command::helper(state.album, msg, client).await?;
    } else if msg.content.len() > 1 && msg.content.starts_with("!") {
        command::picture_find_and_send(state.album, msg, client).await?;
    } else if msg
        .mentions
        .iter()
        .filter(|mention| mention.id == PROTECTED_USER_ID)
        .count()
        != 0
        && !is_admin(&msg, &state).await?
    {
        yell_on_forbidden_mention(msg, client, state).await?;
    }
    Ok(())
}

async fn yell_on_forbidden_mention(
    msg: Box<MessageCreate>,
    client: Arc<HttpClient>,
    state: BotState,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Ok(mut lock) = state.pause_yell.lock() {
        if let Some(pause) = *lock {
            if pause.elapsed() < std::time::Duration::from_secs(3600 * 2) {
                println!("no need to yell on {} while on pause", msg.author.name);
                return Ok(());
            } else {
                println!("pause has ended, resuming normal yelling");
                *lock = None;
            }
        }
    }
    let emoji = twilight_http::request::channel::reaction::RequestReactionType::Custom {
        id: BAN_EMOJI_ID,
        name: Some("ban"),
    };
    client
        .create_reaction(msg.channel_id, msg.id, &emoji)
        .await?;
    client
                .create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "<:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871>",
                )?
                .await?;
    client
                .create_message(msg.channel_id)
                .content("Attention !!!\nIl ne faut pas mentionner Julia, parce que les mentions Discord ça peut vite devenir vraiment très relou.\n\nSi vous répondez a un de ses messages, cliquez toujours sur \"@ ACTIVÉ\" (au dessus à droite de la boite de texte) avant l'envoi pour qu'il affiche \"@ DÉSACTIVÉ\"\n\nNE SUPPRIMEZ PAS VOTRE MESSAGE c'est encore pire de recevoir une mention et de ne pas pouvoir retrouver le message d'où elle provient.")?
                .await?;
    client
                .create_message(msg.channel_id)
                .content(
                    "<:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871><:ban:519852990119673871>",
                )?
                .await?;
    Ok(())
}

async fn admin_guard(
    msg: &Box<twilight_model::gateway::payload::incoming::MessageCreate>,
    state: &BotState,
    client: &Arc<HttpClient>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let is_adm = is_admin(msg, state).await?;

    if !is_adm {
        client
            .create_message(msg.channel_id)
            .reply(msg.id)
            .content("Seul un·e admin peut faire ceci.")?
            .await?;
    }
    Ok(is_adm)
}

async fn is_admin(
    msg: &Box<twilight_model::gateway::payload::incoming::MessageCreate>,
    state: &BotState,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    Ok(if let Some(member) = &msg.member {
        let first_admin_role = member
            .roles
            .iter()
            .find(|item| state.admin_roles.contains(item));
        if let Some(_) = first_admin_role {
            true
        } else {
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
