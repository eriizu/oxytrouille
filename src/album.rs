use std::io::Read;

use multimap::MultiMap;

use rand::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Album {
    pictures: MultiMap<String, String>,
    last_sent: Option<String>,
}

impl Album {
    pub fn new() -> Self {
        Album {
            pictures: MultiMap::new(),
            last_sent: None,
        }
    }

    pub fn from_file(path: &str) -> anyhow::Result<Album> {
        let file = std::fs::File::open(path)?;
        let album: Album = serde_json::from_reader(&file)?;
        return Ok(album);
    }

    pub fn get_rand_pic(self: &mut Self, deck_name: &str) -> Option<&str> {
        match self.pictures.get_vec(deck_name) {
            Some(deck) if deck.len() > 0 => {
                let mut rng = thread_rng();
                let n = rng.gen_range(0..deck.len());
                Some(&deck[n])
            }
            _ => None,
        }
    }

    pub fn add_picture(self: &mut Self, deck_name: &str, picture_link: &str) {
        self.pictures
            .insert(deck_name.to_owned(), picture_link.to_owned());
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
        album.last_sent = Some("http://example.com/riri1.png".to_owned());
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
}
