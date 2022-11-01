use std::io::Write;

use multimap::MultiMap;

use rand::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ErrorKind {
    NotSourced,
}

impl ErrorKind {
    fn to_string(&self) -> &str {
        match self {
            Self::NotSourced => {
                "Album was not sourced from a file and cannot automatically be saved to one."
            }
        }
    }
}

impl std::error::Error for ErrorKind {
    fn description(&self) -> &str {
        self.to_string()
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Album error: {}", self.to_string())
    }
}

#[derive(Serialize, Deserialize)]
struct Picture {
    deck: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
pub struct Album {
    pictures: MultiMap<String, String>,
    last_sent: Option<Picture>,
    #[serde(skip)]
    source_file: Option<String>,
}

impl Album {
    pub fn new() -> Self {
        Album {
            pictures: MultiMap::new(),
            last_sent: None,
            source_file: None,
        }
    }

    pub fn from_file(path: &str) -> anyhow::Result<Album> {
        let file = std::fs::File::open(path)?;
        let mut album: Album = serde_json::from_reader(&file)?;
        album.source_file = Some(path.to_owned());
        return Ok(album);
    }

    pub fn save(self: &Self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.source_file {
            Some(file_name) => {
                let mut file = std::fs::File::create(&file_name)?;
                let to_write = serde_json::to_string_pretty(&self)?;
                writeln!(&mut file, "{}", to_write)?;
                Ok(())
            }
            None => Err(Box::new(ErrorKind::NotSourced)),
        }
    }

    pub fn get_rand_pic(self: &mut Self, deck_name: &str) -> Option<&str> {
        match self.pictures.get_vec(deck_name) {
            Some(deck) if deck.len() > 0 => {
                let mut rng = thread_rng();
                let n = rng.gen_range(0..deck.len());
                self.last_sent = Some(Picture {
                    deck: deck_name.to_string(),
                    url: deck[n].to_owned(),
                });
                Some(&deck[n])
            }
            _ => None,
        }
    }

    pub fn add_picture(self: &mut Self, deck_name: &str, picture_link: &str) {
        self.pictures
            .insert(deck_name.to_owned(), picture_link.to_owned());
    }

    fn deck_picture_remove(
        pictures: &mut MultiMap<String, String>,
        deck_name: &str,
        picture_link: &str,
    ) {
        if let Some(deck) = pictures.remove(deck_name) {
            let deck: Vec<String> = deck
                .iter()
                .filter_map(|val| {
                    if val != picture_link {
                        Some(val.to_owned())
                    } else {
                        None
                    }
                })
                .collect();
            if deck.len() != 0 {
                pictures.insert_many(deck_name.to_owned(), deck);
            }
        }
    }

    pub fn remove_picture(self: &mut Self, deck: &str, url: &str) {
        Self::deck_picture_remove(&mut self.pictures, deck, url);
    }

    pub fn remove_last(self: &mut Self) {
        if let Some(picture) = &self.last_sent {
            Self::deck_picture_remove(&mut self.pictures, &picture.deck, &picture.url);
        }
    }

    pub fn deck_count(self: &Self) -> usize {
        self.pictures.len()
    }

    pub fn picture_count(self: &Self) -> usize {
        self.pictures.iter_all().map(|(_, deck)| deck.len()).sum()
    }
}

impl Default for Album {
    fn default() -> Self {
        let mut album = Self::new();
        album.add_picture("mood", "http://example.com/mood1.png");
        album.add_picture("mood", "http://example.com/mood2.png");
        album.add_picture("mood", "http://example.com/mood3.png");
        album.add_picture("tata", "http://example.com/tata.png");
        album.add_picture("riri", "http://example.com/riri1.png");
        album.add_picture("riri", "http://example.com/riri2.png");
        album.last_sent = Some(Picture {
            deck: "riri".to_owned(),
            url: "http://example.com/riri1.png".to_owned(),
        });
        return album;
    }
}

#[cfg(test)]
mod tests {
    use super::Album;

    #[test]
    fn deck_count() {
        let album = Album::default();

        assert_eq!(album.deck_count(), 3);
    }
    #[test]
    fn picture_count() {
        let album = Album::default();

        assert_eq!(album.picture_count(), 6);
    }

    #[test]
    fn get_rand_pic_no_match() {
        let mut album = Album::default();

        let link = album.get_rand_pic("not_matching_key");
        assert_eq!(link, None);
    }

    #[test]
    fn get_rand_pic_mood() {
        let mut album = Album::default();

        let link = album.get_rand_pic("mood").unwrap();
        assert!(link.contains("mood"));
        let link = album.get_rand_pic("mood").unwrap();
        assert!(link.contains("mood"));
        let link = album.get_rand_pic("mood").unwrap();
        assert!(link.contains("mood"));
    }

    #[test]
    fn get_rand_pic_tata() {
        let mut album = Album::default();

        let link = album.get_rand_pic("tata").unwrap();
        assert!(link.contains("tata"));
        let link = album.get_rand_pic("tata").unwrap();
        assert!(link.contains("tata"));
        let link = album.get_rand_pic("tata").unwrap();
        assert!(link.contains("tata"));
    }

    #[test]
    fn get_rand_pic_riri() {
        let mut album = Album::default();

        let link = album.get_rand_pic("riri").unwrap();
        assert!(link.contains("riri"));
        let link = album.get_rand_pic("riri").unwrap();
        assert!(link.contains("riri"));
        let link = album.get_rand_pic("riri").unwrap();
        assert!(link.contains("riri"));
    }

    #[test]
    fn remove_last() {
        let mut album = Album::default();
        let old_len = album.picture_count();

        album.get_rand_pic("tata").unwrap();
        album.remove_last();
        match album.get_rand_pic("tata") {
            Some(_) => panic!(),
            _ => {}
        }
        assert!(album.picture_count().eq(&(old_len - 1)));
    }
}
