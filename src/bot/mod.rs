use futures::stream::StreamExt;
use std::{env, error::Error, sync::Arc, sync::Mutex};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, Event};
use twilight_http::Client as HttpClient;
use twilight_model::{application, application::interaction};

mod command;

pub async fn start(alb: crate::album::Album) -> anyhow::Result<()> {
    let token = env::var("DISCORD_TOKEN")?;
    let alb = Arc::new(Mutex::new(alb));

    let tmp = Arc::clone(&alb);
    let _ = ctrlc::set_handler(move || {
        eprintln!("stoping...");
        match tmp.lock() {
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

    slash_command_trial(&client).await?;

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
    let tmp = interact_client.create_guild_command(ID);

    let chat_input = tmp.chat_input("add", "adds a picture to an album").unwrap();
    let options = vec![
        application::command::CommandOption::Attachment(
            application::command::BaseCommandOptionData {
                description: "image to add".to_owned(),
                name: "image".to_owned(),
                required: true,
                description_localizations: None,
                name_localizations: None,
            },
        ),
        application::command::CommandOption::String(
            application::command::ChoiceCommandOptionData {
                description: "deck name".to_owned(),
                name: "deck".to_owned(),
                required: true,
                description_localizations: None,
                name_localizations: None,
                max_length: None,
                min_length: None,
                choices: vec![],
                autocomplete: false,
            },
        ),
    ];
    chat_input.command_options(&options)?.exec().await?;

    let commands = client
        .interaction(application_id)
        .guild_commands(ID)
        .exec()
        .await?
        .models()
        .await?;
    println!("there are {} guild commands", commands.len());
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
        Event::MessageCreate(msg) if msg.content.len() > 1 => {
            command::picture_find_and_send(album, msg, client).await?;
        }
        Event::ShardConnected(_) => {
            println!("Connected on shard {shard_id}");
        }
        Event::InteractionCreate(interact) => {
            match interact.0.data {
                Some(interaction::InteractionData::ApplicationCommand(command)) => {
                    // need to store image path, and deck in concerns
                    // image needs to be downloaded and resent

                    // println!("I have a command!!!!! {}", command.name);
                    println!("{:#?}", command);
                    match command.resolved {
                        Some(resolved) => {
                            for (_, att) in resolved.attachments {
                                let b = reqwest::get(att.url).await?.bytes().await?.to_vec();
                                if let Some(chan_id) = interact.0.channel_id {
                                    let tmp =
                                        twilight_model::http::attachment::Attachment::from_bytes(
                                            "test.png".to_owned(),
                                            b,
                                            1,
                                        );
                                    client
                                        .create_message(chan_id)
                                        .attachments(&vec![tmp])?
                                        .exec()
                                        .await;
                                }
                            }
                        }
                        _ => {}
                    }
                    for opt in command.options {
                        println!("{:#?}", opt);
                        match opt.value {
                            interaction::application_command::CommandOptionValue::Attachment(
                                value,
                            ) => {
                                println!("attachment {}", value);
                                // if let Some(chan_id) = interact.channel_id {
                                //     // let tmp =
                                //     //     twilight_model::http::attachment::Attachment::from_bytes();

                                //     client.create_message(chan_id).attachments(&vec![]);
                                // }
                            }
                            interaction::application_command::CommandOptionValue::String(value) => {
                                println!("str {}", value);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            match interact.0.kind {
                interaction::InteractionType::ApplicationCommand => {}
                _ => {}
            }
            // Other events here...
        }
        _ => {}
    }

    Ok(())
}
