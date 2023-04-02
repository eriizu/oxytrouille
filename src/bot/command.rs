use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use twilight_http::Client as HttpClient;
use twilight_model::channel::message::AllowedMentions;
use twilight_model::gateway::payload::incoming::MessageCreate;

async fn reply_in_chann(
    http: &Arc<HttpClient>,
    msg: Box<MessageCreate>,
    response: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mentions = AllowedMentions::builder().build();
    http.create_message(msg.channel_id)
        .allowed_mentions(Some(&mentions))
        .reply(msg.id)
        .content(response)?
        .exec()
        .await?;
    Ok(())
}

pub async fn picture_find_and_send(
    album: Arc<Mutex<crate::album::Album>>,
    msg: Box<MessageCreate>,
    http: Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let link = if let Some(deck_name) = msg.content.strip_prefix("!") {
        match album.lock() {
            Ok(mut album) => {
                if let Some(link) = album.get_rand_pic(deck_name) {
                    Some(link.to_owned())
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };
    Ok(match link {
        Some(link) => {
            reply_in_chann(&http, msg, &link).await?;
        }
        _ => {}
    })
}

fn mk_names_str(mut deck_names: Vec<&String>) -> String {
    deck_names.sort();
    let mut names_str = String::new();
    let mut first = true;
    for name in deck_names {
        if first {
            first = false;
        } else {
            names_str.push_str(", ");
        }
        names_str.push_str(name);
    }
    return names_str;
}

pub async fn helper(
    album: Arc<Mutex<crate::album::Album>>,
    msg: Box<MessageCreate>,
    http: Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let reply = match album.lock() {
        Ok(album) => Some(format!(
            "Nombre d'albums: {}, nombre de photos: {}.\nNom des albums: {}.",
            album.deck_count(),
            album.picture_count(),
            mk_names_str(album.deck_names().collect())
        )),
        Err(_) => None,
    };
    if let Some(reply) = reply {
        reply_in_chann(&http, msg, &reply).await?;
    }
    Ok(())
}

pub async fn picture_add(
    msg: Box<MessageCreate>,
    album: &Arc<Mutex<crate::album::Album>>,
    http: &Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut num_added = 0;
    let mut split = msg.content.split(' ');
    let mut response = "Je n'ai rien trouvé en pièce jointe a ajouter.";
    split.next();
    if let Some(deck_name) = split.next() {
        match album.lock() {
            Ok(mut album) => {
                for att in &msg.attachments {
                    album.add_picture(deck_name, &att.url);
                    num_added += 1;
                }
                match album.save() {
                    Ok(_) => println!("album save sucessful"),
                    Err(_) => eprintln!("failed to save album, data loss is possible"),
                }
            }
            Err(_) => response = "Je n'arrive pas à modifier l'album, je pense que vous pouvez essayer à nouveau dans quelques minutes.",
        }
    }
    if num_added > 0 {
        let response = format!("J'ai ajouté {} image·s !", num_added);
        reply_in_chann(http, msg, &response).await?;
    } else {
        reply_in_chann(http, msg, response).await?;
    }
    Ok(())
}

pub async fn delete_last(
    msg: Box<MessageCreate>,
    album: &Arc<Mutex<crate::album::Album>>,
    http: &Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let response =
        "Je ne me souvient pas de la dernière image envoyée, donc je n'ai rien supprimé.";

    let removed = match album.lock() {
        Ok(mut album) => album.remove_last(),
        Err(_) => None,
    };

    if let Some(removed) = removed {
        let response = format!(
            "Depuis le deck {} j'ai supprimé l'image {}",
            removed.deck, removed.url
        );
        reply_in_chann(http, msg, &response).await?;
    } else {
        reply_in_chann(http, msg, response).await?;
    }

    Ok(())
}

pub async fn delete_picture(
    msg: Box<MessageCreate>,
    album: &Arc<Mutex<crate::album::Album>>,
    http: &Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut response = "Je n'ai rien supprimé.";
    let mut split = msg.content.split(' ');

    split.next();
    let removed = if let Some(deck_name) = split.next() {
        if let Some(url) = split.next() {
            match album.lock() {
                Ok(mut album) => album.remove_picture(deck_name, url),
                Err(_) => false,
            }
        } else {
            false
        }
    } else {
        false
    };

    if removed {
        response = "J'ai supprimé l'image !";
    }
    reply_in_chann(http, msg, response).await?;
    Ok(())
}

async fn get_gild_members(
    guild_id: &twilight_model::id::Id<twilight_model::id::marker::GuildMarker>,
    http: &Arc<HttpClient>,
) -> Result<Vec<twilight_model::guild::member::Member>, &'static str> {
    let Ok(resp) = http.guild_members(*guild_id).exec().await else {
        return Err("api didn't respond with the member list");
    };

    let Ok(members) = resp.models().await else {
        return Err("could not make member models out of api response");
    };
    return Ok(members);
}

async fn member_reset_nickname(
    http: &Arc<HttpClient>,
    guild_id: twilight_model::id::Id<twilight_model::id::marker::GuildMarker>,
    user_id: twilight_model::id::Id<twilight_model::id::marker::UserMarker>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut update_builder = http.update_guild_member(guild_id, user_id);
    update_builder = update_builder.nick(None)?;
    update_builder.exec().await?;
    Ok(())
}
pub async fn reset_nick(
    msg: Box<MessageCreate>,
    http: &Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut split = msg.content.split(' ');
    split.next();
    let to_reset: Vec<&str> = split.collect();
    let Some(guild_id) = msg.guild_id else {
        reply_in_chann(http, msg, "Je n'ai pas réussi à récupérer l'identifiant de la guilde").await?;
        return Ok(());
    };
    let resp = http
        .guild_members(guild_id)
        .limit(1000)?
        .exec()
        .await
        .unwrap();
    let members = resp.models().await.unwrap();
    //   let Ok(members) = get_gild_members(&guild_id, http).await else {
    //       reply_in_chann(http, msg, "Je n'ai pas réussi à récupérer la liste de membres").await?;
    //       return Ok(());
    //   };
    let members_to_reset = members.iter().filter(|item| match &item.nick {
        Some(nick) if to_reset.contains(&(nick as &str)) => true,
        _ => false,
    });

    let mut changed_str: String = String::new();
    let mut failed_str: String = String::new();
    let mut response = "J'ai mis à zéro les noms d'utilisateur de :".to_owned();
    for member in members_to_reset {
        match member_reset_nickname(http, guild_id, member.user.id).await {
            Ok(_) => changed_str.push_str(&format!(
                " {}#{}",
                member.user.name, member.user.discriminator
            )),
            Err(err) => {
                failed_str.push_str(&format!(
                    " {}#{}",
                    member.user.name, member.user.discriminator
                ));
                eprintln!("failed to change nickanme because: {}", err)
            }
        }
    }
    response.push_str(&changed_str);
    if failed_str.len() > 0 {
        response.push_str("\nJe n'ai pas réussi à changer ceux de :");
        response.push_str(&failed_str);
    }
    reply_in_chann(http, msg, &response).await?;
    Ok(())
}