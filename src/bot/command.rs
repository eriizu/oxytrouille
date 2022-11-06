use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use twilight_http::Client as HttpClient;
use twilight_model::gateway::payload::incoming::MessageCreate;

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
            http.create_message(msg.channel_id)
                .content(&link)?
                .exec()
                .await?;
        }
        _ => {}
    })
}

pub async fn helper(
    album: Arc<Mutex<crate::album::Album>>,
    msg: Box<MessageCreate>,
    http: Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let reply = match album.lock() {
        Ok(album) => {
            let deck_count = album.deck_count();
            let pic_count = album.picture_count();
            let mut deck_names: Vec<&String> = album.deck_names().collect();
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
            Some(format!(
                "Nombre d'albums: {}, nombre de photos: {}.\nNom des albums: {}.",
                deck_count, pic_count, names_str
            ))
        }
        Err(_) => None,
    };
    if let Some(reply) = reply {
        http.create_message(msg.channel_id)
            .reply(msg.id)
            .content(&reply)?
            .exec()
            .await?;
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
    let mut response = "Je n'ai rien trouvé à ajouter.".to_owned();
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
            Err(_) => response = "Je n'ai pas réussi a gagner l'accès à l'album.".to_owned(),
        }
    }
    if num_added > 0 {
        response = format!("J'ai ajouté {} image·s !", num_added);
    }
    http.create_message(msg.channel_id)
        .content(&response)?
        .exec()
        .await?;
    Ok(())
}

pub async fn delete_last(
    msg: Box<MessageCreate>,
    album: &Arc<Mutex<crate::album::Album>>,
    http: &Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let response = "rine".to_owned();

    match album.lock() {
        Ok(mut album) => album.remove_last(),
        Err(_) => {}
    }

    http.create_message(msg.channel_id)
        .content(&response)?
        .exec()
        .await?;
    Ok(())
}

pub async fn delete_picture(
    msg: Box<MessageCreate>,
    album: &Arc<Mutex<crate::album::Album>>,
    http: &Arc<HttpClient>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let response = "rine".to_owned();
    let mut split = msg.content.split(' ');

    split.next();
    let deck_name = split.next().unwrap();
    let url = split.next().unwrap();
    match album.lock() {
        Ok(mut album) => album.remove_picture(deck_name, url),
        Err(_) => {}
    }

    http.create_message(msg.channel_id)
        .content(&response)?
        .exec()
        .await?;
    Ok(())
}
