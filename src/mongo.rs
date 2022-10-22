use futures::stream::TryStreamExt;
use mongodb::{bson::doc, options::ClientOptions, Client};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PictureAuthor {
    id: String,
    tag: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Picture {
    link: String,
    validated: bool,
    author: PictureAuthor,
    album: String,
    // #[serde(rename = "createdOn")]
    // created_on: String,
    // #[serde(rename = "updatedOn")]
    // updated_on: String,
}

pub async fn connect_and_pull() -> anyhow::Result<crate::album::Album> {
    let conn_str = std::env::var("MONGO_URL")?;

    // Parse a connection string into an options struct.
    let client_options = ClientOptions::parse(conn_str).await?;

    // Manually set an option.
    // client_options.app_name = Some("oxytrouille".to_string());

    // Get a handle to the deployment.
    let client = Client::with_options(client_options)?;

    // List the names of the databases in that deployment.
    for db_name in client.list_database_names(None, None).await? {
        println!("{}", db_name);
    }

    let db = client.database("citrouille");

    let collection = db.collection::<Picture>("pictures");

    let mut cursor = collection.find(doc! {"validated": true}, None).await?;

    let mut alb = crate::album::Album::new();
    while let Some(entry) = cursor.try_next().await? {
        println!("{:?}", entry);
        alb.add_picture(&entry.album, &entry.link);
    }

    Ok(alb)
}
