use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use twilight_http::Client as HttpClient;
use twilight_model::gateway::payload::incoming::MessageCreate;

async fn reply_in_chann(
    http: &Arc<HttpClient>,
    msg: Box<MessageCreate>,
    response: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    http.create_message(msg.channel_id)
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
    let response = "Je ne me souvient pas de la dernière image envoyée, je n'ai rien supprimé.";

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
