use std::collections::VecDeque;

use regex::Regex;
use serde::Deserialize;

use crate::{
    config::{Configuration, Key},
    gallery::prelude::*,
};

pub fn extract(url: &str) -> crate::Result<GelbooruGallery> {
    let config = Configuration::init();
    let user_id = config.get_config(Key::GelbooruUser)?.into();

    // The user-supplied URL will presumably be copied from the web interface, but we are
    // really not interested in the url itself. We pretty much only want the search tags.
    let tags = read_tags(url)?.into();

    // Once we have the search tags, we need... Fuck all. Let's go.
    Ok(GelbooruGallery {
        user_id,
        client: build_client()?,
        tags,
        queue: VecDeque::new(),
        page: 0,
        is_complete: false,
    })
}

pub struct GelbooruGallery {
    user_id: String,
    client: Client,
    tags: String,
    queue: VecDeque<Image>,
    page: usize,
    is_complete: bool,
}

impl GelbooruGallery {
    fn retrieve_batch(&mut self) -> crate::Result<usize> {
        let request = Request {
            user_id: &self.user_id,
            tags: &self.tags,
            pid: self.page,
        };

        let queue: VecDeque<Image> = self.client.get(&request.format()).send()?.json()?;

        self.queue = queue;
        self.page += 1;

        Ok(self.queue.len())
    }
}

impl Gallery for GelbooruGallery {
    fn advance_by(&mut self, mut skip: usize) -> crate::Result<()> {
        loop {
            if skip == 0 {
                return Ok(());
            }

            if skip < self.queue.len() {
                self.queue.drain(..skip);
                return Ok(());
            }

            skip = skip.saturating_sub(self.queue.len());
            self.queue.clear();
            if 0 == self.retrieve_batch()? {
                self.is_complete = true;
                return Ok(());
            }
        }
    }

    fn next(&mut self) -> Option<crate::Result<GalleryItem>> {
        if self.is_complete {
            return None;
        }

        if self.queue.is_empty() {
            match self.retrieve_batch() {
                // API returns empty array for nonexistent pages.
                Ok(0) => {
                    self.is_complete = true;
                    return None;
                }

                Err(e) => {
                    self.is_complete = true;
                    return Some(Err(e));
                }

                _ => (),
            }
        }

        let image = self.queue.pop_front()?;
        match self.client.get(&image.file_url).send() {
            Ok(response) => Some(Ok(GalleryItem::new(image.file_url, response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Image {
    // This ID serves no purpose that I'm aware of just yet, but... Meh. Whatever, ok?
    id: u32,
    file_url: String,
}

// https://gelbooru.com/index.php?page=dapi&s=post&q=index&limit=10&tags=loli+slave+sweat+whip_marks&json=1&pid=1
struct Request<'a> {
    user_id: &'a str,
    tags: &'a str,
    pid: usize,
}

impl Request<'_> {
    // I originally had a custom Serialize implementation for this struct. That did not work
    // because it automatically escapes plus signs, which must be retained intact in order for
    // the damned API to work.
    fn format(&self) -> String {
        format!(
            "https://gelbooru.com/index.php?api_key=anonymous&user_id={}&page=dapi&s=post&q=index&limit=100&tags={}&pid={}&json=1",
            self.user_id,
            self.tags,
            self.pid
        )
    }
}

fn read_tags(url: &str) -> crate::Result<&str> {
    let pattern = Regex::new(r#"tags=([^&]+)"#).unwrap();
    Ok(pattern
        .captures(url)
        .ok_or_else(|| Error::Unsupported(UnsupportedError::Route, url.into()))?
        .get(1)
        .unwrap()
        .as_str())
}

fn build_client() -> crate::Result<Client> {
    use reqwest::header;

    let builder = Client::builder();
    let mut headers = header::HeaderMap::new();

    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("text/html"),
    );
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(super::USER_AGENT),
    );

    Ok(builder.default_headers(headers).build()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn can_deserialize() -> serde_json::Result<()> {
        let payload = include_str!("../../resource/gelbooru/search.json");
        let result: Vec<super::Image> = serde_json::from_str(payload)?;
        assert_eq!(10, result.len());
        Ok(())
    }

    #[test]
    fn can_read_tags() -> crate::Result<()> {
        let url = "https://gelbooru.com/index.php?page=post&s=list&tags=slave+loli";
        assert_eq!("slave+loli", super::read_tags(url)?);
        Ok(())
    }
}
