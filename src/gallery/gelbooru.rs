use regex::Regex;
use serde::Deserialize;

use crate::config::{Configuration, Key};

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<PagedGallery<GelbooruPager>> {
    let config = Configuration::init();
    let user_id = config.get_config(Key::GelbooruUser)?.into();

    // The user-supplied URL will presumably be copied from the web interface, but we are
    // really not interested in the url itself. We pretty much only want the search tags.
    let tags = read_tags(url)?.into();

    Ok(PagedGallery {
        context: build_client(),
        pager: GelbooruPager {
            user_id,
            tags,
            page: 0,
            is_complete: false,
        },
        current: Page::Empty,
    })
}

pub struct GelbooruPager {
    user_id: String,
    tags: String,
    page: usize,
    is_complete: bool,
}

impl Pager for GelbooruPager {
    type Context = Client;

    type Item = Image;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        if self.is_complete {
            return Ok(Page::Empty);
        }

        let request = Request {
            user_id: &self.user_id,
            tags: &self.tags,
            pid: self.page,
        };
        self.page += 1;

        let page: VecDeque<Image> = context.get(&request.format()).send()?.json()?;
        if page.is_empty() {
            self.is_complete = true;
            Ok(Page::Empty)
        } else {
            Ok(Page::Items(page))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Image {
    // This ID serves no purpose that I'm aware of just yet, but...
    // Meh. Whatever, ok?
    id: u32,
    file_url: String,
}

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

impl Downloadable for Image {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(context
            .get(&self.file_url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

fn build_client() -> Client {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("text/html"));
    Client::builder()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .unwrap()
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

#[cfg(test)]
mod tests {
    use super::Image;

    #[test]
    fn can_deserialize() -> serde_json::Result<()> {
        let payload = include_str!("../../resource/gelbooru/search.json");
        let result: Vec<Image> = serde_json::from_str(payload)?;
        assert_eq!(10, result.len());
        Ok(())
    }

    #[test]
    fn can_read_tags() -> crate::Result<()> {
        let url = "https://gelbooru.com/index.php?page=post&s=list&tags=text+tags";
        assert_eq!("text+tags", super::read_tags(url)?);
        Ok(())
    }
}
