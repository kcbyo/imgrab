//! Bleachbooru has 40 images per page.
//!
//! Preview and image links are the same except for "preview" and "image"
//!
//! https://bleachbooru.org/data/image/75/54/7554f10d5843803eaf9e4d4b90903785.jpg
//! https://bleachbooru.org/data/preview/75/54/7554f10d5843803eaf9e4d4b90903785.jpg
//!
//! Even crazier:
//! https://bleachbooru.org/help/api
//!
//! Apparently one may log in using a username and *hashed password.* Which is wild.
//! The "hash" is the sha1 hash of the password and a salt, of the form:
//!
//! choujin-steiner--<password>--
//!
//! ...which is the most insane fucking thing I've ever heard.
//!
//! Apparently the basic thing you do is post to
//!
//! /post.xml
//!
//! ...Will that spit out json?
//!
//! No, it doesn't, but changing it to /post.json will. The API does not respect the
//! accept header.
//!
//! So, yeah, also, you log in by adding your username and the hashword to the request
//! as parameters. Fun fun fun.

use serde::{Deserialize, Serialize};

use crate::config::{Configuration, Key};

use super::prelude::*;

static BASE_URL: &str = "https://bleachbooru.org/post.json";

static IMAGE_BASE_URL: &str = "https://bleachbooru.org";

pub fn extract(url: &str) -> crate::Result<(PagedGallery<BleachbooruPager>, Option<String>)> {
    let config = Configuration::init();
    let auth = Auth {
        username: config.get_config(Key::BleachUser)?.into(),
        password_hash: apply_salt_and_hash(config.get_config(Key::BleachPass)?),
    };

    let gallery = PagedGallery {
        context: super::build_client(),
        pager: BleachbooruPager {
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

fn apply_salt_and_hash(password: &str) -> String {
    static HEAD: &str = "choujin-steiner--";
    static TAIL: &str = "--";

    let mut m = sha1_smol::Sha1::new();
    m.update(HEAD.as_bytes());
    m.update(password.as_bytes());
    m.update(TAIL.as_bytes());
    m.digest().to_string()
}

pub struct BleachbooruPager {
    auth: Auth,
    tags: String,
    page: usize,
    is_complete: bool,
}

impl Pager for BleachbooruPager {
    type Context = Client;

    type Item = Image;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        if self.is_complete {
            return Ok(Page::Empty);
        }

        let request = Request {
            auth: &self.auth,
            limit: 100,
            page: self.page,
            tags: &self.tags,
        };
        self.page += 1;

        let images: VecDeque<Image> = context.get(request.format()).send()?.json()?;
        if !images.is_empty() {
            Ok(Page::Items(images))
        } else {
            self.is_complete = true;
            Ok(Page::Empty)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Image {
    // id: i64,
    // tags: String,
    // author: String,
    // source: String,
    // score: i64,
    // md5: String,
    // file_size: i64,
    file_url: String,
    // rating: String,
    // status: String,
}

impl Downloadable for Image {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let url = format!("{IMAGE_BASE_URL}{}", self.file_url);
        Ok(context.get(url).send().map(ResponseGalleryItem::new)?)
    }
}

#[derive(Debug, Serialize)]
struct Request<'a> {
    #[serde(flatten)]
    auth: &'a Auth,
    limit: usize,
    page: usize,
    tags: &'a str,
}

#[derive(Debug, Serialize)]
struct Auth {
    username: String,
    password_hash: String,
}

impl Request<'_> {
    fn format(&self) -> String {
        let username = &self.auth.username;
        let password = &self.auth.password_hash;
        let limit = self.limit;
        let page = self.page;
        let tags = self.tags;

        format!("{BASE_URL}?username={username}&password_hash={password}&limit={limit}&page={page}&tags={tags}")
    }
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
