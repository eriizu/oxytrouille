use multimap::MultiMap;

use rand::prelude::*;

struct Album {
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
        album
            .pictures
            .insert("mood".to_owned(), "http://example.com/mood1.png".to_owned());
        album
            .pictures
            .insert("mood".to_owned(), "http://example.com/mood2.png".to_owned());
        album
            .pictures
            .insert("mood".to_owned(), "http://example.com/mood3.png".to_owned());
        album
            .pictures
            .insert("tata".to_owned(), "http://example.com/tata.png".to_owned());
        album
            .pictures
            .insert("riri".to_owned(), "http://example.com/riri1.png".to_owned());
        album
            .pictures
            .insert("riri".to_owned(), "http://example.com/riri2.png".to_owned());
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
