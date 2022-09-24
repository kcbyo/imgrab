use std::{collections::HashMap, fs};

use directories::UserDirs;

use crate::{error::Error, Result};

#[derive(Clone, Debug, Default)]
pub struct Configuration {
    // text: String,
    config: HashMap<Key, String>,
}

impl Configuration {
    /// Constructs a new config provider based on a provided path.
    ///
    /// Should there be no configuration at the provided path, a blank
    /// configuration provider will be produced.
    pub fn init() -> Self {
        let text = UserDirs::new()
            .map(|dirs| dirs.home_dir().join(".imgrab.conf"))
            .and_then(|conf| fs::read_to_string(&conf).ok());

        text.map(|text| Configuration {
            config: read_config(&text),
            // I think this was originally used for debugging, but I have no
            // use for it right now
            // text,
        })
        .unwrap_or_else(Default::default)
    }

    pub fn get_config(&self, key: Key) -> Result<&str> {
        self.config
            .get(&key)
            .map(AsRef::as_ref)
            .ok_or(Error::Configuration(key))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Key {
    AtfBooruApi,
    AtfBooruUser,
    BleachUser,
    BleachPass,
    EHentaiPass,
    EHentaiUser,
    GelbooruUser,
    ImgurClientId,
    SankakuPass,
    SankakuUser,
}

impl Key {
    fn from_identifier(identifier: &str) -> Option<Self> {
        match identifier {
            "atfb_api" => Some(Key::AtfBooruApi),
            "atfb_user" => Some(Key::AtfBooruUser),
            "bleach_username" => Some(Key::BleachUser),
            "bleach_password" => Some(Key::BleachPass),
            "ehentai_password" => Some(Key::EHentaiPass),
            "ehentai_username" => Some(Key::EHentaiUser),
            "gelbooru_user" => Some(Key::GelbooruUser),
            "imgur_client_id" => Some(Key::ImgurClientId),
            "sankaku_password" => Some(Key::SankakuPass),
            "sankaku_username" => Some(Key::SankakuUser),
            _ => None,
        }
    }
}

fn read_config(text: &str) -> HashMap<Key, String> {
    text.lines()
        .filter_map(|line| {
            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            let mut parts = line.split('=');
            let key = parts.next()?;
            let value = parts.next()?;
            Key::from_identifier(key).map(|key| (key, value.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{read_config, Key};

    static CONTENT: &str = "ehentai_username=foo\n\
                            \n\
                            # Comment\n\
                            ehentai_password=bar\n\
                            gelbooru_user=1234\n\
                            imgur_client_id=baz\n";

    #[test]
    fn can_extract_config() {
        let config = read_config(CONTENT);
        assert_eq!("foo", config[&Key::EHentaiUser]);
        assert_eq!("bar", config[&Key::EHentaiPass]);
        assert_eq!("1234", config[&Key::GelbooruUser]);
        assert_eq!("baz", config[&Key::ImgurClientId]);
    }
}
