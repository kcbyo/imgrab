use serde::{Deserialize, Serialize};

use crate::{
    config::{Configuration, Key},
    gallery::prelude::*,
    tags::Tags,
};

#[derive(Clone, Debug, Serialize)]
struct LoginRequest<'a> {
    login: &'a str,
    password: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
struct LoginResponse {
    success: bool,
    token_type: String,
    access_token: String,
    refresh_token: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PageResponse {
    meta: PageResponseMetadata,
    data: VecDeque<Image>,
}

#[derive(Clone, Debug, Deserialize)]
struct PageResponseMetadata {
    next: Option<String>,
    prev: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Image {
    id: i64,
    rating: String,
    file_url: String,
    width: i32,
    height: i32,
    file_size: i32,
    file_type: String,
    md5: String,
}

// https://beta.sankakucomplex.com/?tags=slave%20sweat%20welts

pub fn extract(url: &str) -> crate::Result<PagedGallery<SankakuPager>> {
    // I doubt we'll see hashes at the end of these urls, but who knows?
    let url = url.trim_end_matches('#');
    let tags = Tags::try_from_url(url, "%20").ok_or_else(|| {
        Error::Unsupported(
            UnsupportedError::Route,
            String::from("Sankaku urls must have one or more tags"),
        )
    })?;

    // We need to sign in to get the goodies.
    let config = Configuration::init();
    let username = config.get_config(Key::SankakuUser)?;
    let password = config.get_config(Key::SankakuPass)?;
    let client = build_client();

    // This process pulls both the access and refresh token from the login response,
    // but according to my research the access token will last something like 48 hours.
    // In other words, we really have no need of the refresh token.
    let LoginResponse { access_token, .. } = client
        .post("https://capi-v2.sankakucomplex.com/auth/token")
        .json(&LoginRequest {
            login: &username,
            password: &password,
        })
        .send()?
        .json()?;

    Ok(PagedGallery {
        context: Context {
            client,
            token: access_token,
        },
        pager: SankakuPager {
            tags,
            next: None,
            has_started: false,
        },
        current: Page::Empty,
    })
}

pub struct Context {
    client: Client,
    token: String,
}

pub struct SankakuPager {
    tags: Tags,
    next: Option<String>,
    has_started: bool,
}

impl SankakuPager {
    fn next_url(&mut self) -> Option<String> {
        match &self.next {
            Some(next_id) => Some(format!("https://capi-v2.sankakucomplex.com/posts/keyset?lang=en&next={}&default_threshold=1&hide_posts_in_books=never&limit=40&tags={}", next_id, self.tags)),
            None if !self.has_started => {
                self.has_started = true;
                Some(format!("https://capi-v2.sankakucomplex.com/posts/keyset?lang=en&default_threshold=1&hide_posts_in_books=never&limit=40&tags={}", self.tags))
            }
            None => None,
        }
    }
}

impl Pager for SankakuPager {
    type Context = Context;
    type Item = Image;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        let url = match self.next_url() {
            Some(url) => url,
            None => return Ok(Page::Empty),
        };

        let PageResponse { meta, data } = context
            .client
            .get(&url)
            .bearer_auth(&context.token)
            .send()?
            .json()?;

        self.next = meta.next;
        Ok(Page::Items(data))
    }
}

impl Downloadable for Image {
    type Context = Context;
    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(context
            .client
            .get(&self.file_url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

fn build_client() -> Client {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};

    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.sankaku.api+json;v=2"),
    );

    Client::builder()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .default_headers(headers)
        .build()
        .unwrap()
}
