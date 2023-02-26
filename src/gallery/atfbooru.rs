use serde::{Deserialize, Serialize};

use crate::config::{Configuration, Key};

use super::prelude::*;

static BASE_URL: &str = "https://booru.allthefallen.moe/posts.json";

pub fn extract(url: &str) -> crate::Result<(PagedGallery<AtfBooruPager>, Option<String>)> {
    let config = Configuration::init();
    let auth = Auth {
        user: config.get_config(Key::AtfBooruUser)?.into(),
        api_key: config.get_config(Key::AtfBooruApi)?.into(),
    };

    let gallery = PagedGallery {
        context: configure_client(),
        pager: AtfBooruPager {
            auth,
            tags: read_tags(url)?.into(),
            page: 1,
            is_complete: false,
        },
        current: Page::Empty,
    };

    match get_single_tag(&gallery.pager.tags).map(|tag| tag.to_owned()) {
        Some(tag) => Ok((gallery, Some(tag))),
        None => Ok((gallery, None)),
    }
}

struct Auth {
    user: String,
    api_key: String,
}

pub struct AtfBooruPager {
    auth: Auth,
    tags: String,
    page: usize,
    is_complete: bool,
}

impl Pager for AtfBooruPager {
    type Context = Client;

    type Item = Image;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        if self.is_complete {
            return Ok(Page::Empty);
        }

        let request = Request {
            limit: 100,
            page: self.page,
            tags: &self.tags,
        };

        self.page += 1;

        let images: VecDeque<Image> = context
            .get(request.format())
            .basic_auth(&self.auth.user, Some(&self.auth.api_key))
            .send()?
            .json()?;

        if !images.is_empty() {
            Ok(Page::Items(images))
        } else {
            self.is_complete = true;
            Ok(Page::Empty)
        }
    }
}

#[derive(Debug, Serialize)]
struct Request<'a> {
    limit: usize,
    page: usize,
    tags: &'a str,
}

impl Request<'_> {
    fn format(&self) -> String {
        let limit = self.limit;
        let page = self.page;
        let tags = self.tags;
        format!("{BASE_URL}?limit={limit}&page={page}&tags={tags}")
    }
}

#[derive(Debug, Deserialize)]
pub struct Image {
    file_url: String,
}

impl Downloadable for Image {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(context
            .get(self.file_url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

fn configure_client() -> Client {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    Client::builder()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .unwrap()
}

fn read_tags(url: &str) -> crate::Result<&str> {
    let pattern = regex::Regex::new(r#"tags=([^&]+)"#).unwrap();
    Ok(pattern
        .captures(url)
        .ok_or_else(|| Error::Unsupported(UnsupportedError::Route, url.into()))?
        .get(1)
        .unwrap()
        .as_str())
}

fn get_single_tag(tags: &str) -> Option<&str> {
    let mut tags = tags.split('+');
    let single = tags.next();
    single.filter(|_| tags.next().is_none())
}
