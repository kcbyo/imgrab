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
struct Image {
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

pub struct SankakuBeta;

impl ReadGallery for SankakuBeta {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
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
        let client = build_client()?;

        // This process pulls both the access and refresh token from the login response,
        // but according to my research the access token will last something like 48 hours.
        // In other words, we really have no need of the refresh token.
        let LoginResponse {
            access_token,
            refresh_token,
            ..
        } = client
            .post("https://capi-v2.sankakucomplex.com/auth/token")
            .json(&LoginRequest {
                login: &username,
                password: &password,
            })
            .send()?
            .json()?;

        Ok(Box::new(SankakuBetaGallery {
            client: build_client()?,
            tags,
            count: 0,
            queue: VecDeque::new(),
            next: None,
            has_started: false,
            is_complete: false,
            access_token,
            refresh_token,
        }))
    }
}

#[derive(Clone, Debug)]
pub struct SankakuBetaGallery {
    client: Client,
    tags: Tags,
    count: usize,
    next: Option<String>,
    queue: VecDeque<Image>,
    has_started: bool,
    is_complete: bool,

    // No clue what I'm gonna do with these.
    access_token: String,
    refresh_token: String,
}

impl SankakuBetaGallery {
    fn retrieve_batch(&mut self) -> crate::Result<usize> {
        let url = match self.next_url() {
            Some(url) => url,
            None => return Ok(0),
        };

        let PageResponse { meta, data } = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()?
            .json()?;

        self.next = meta.next;
        self.queue = data;
        Ok(self.queue.len())
    }

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

impl Gallery for SankakuBetaGallery {
    fn apply_skip(&mut self, mut skip: usize) -> crate::Result<()> {
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
            match self.retrieve_batch()? {
                0 => {
                    self.is_complete = true;
                    return Ok(());
                }
                count => self.count += count,
            }
        }
    }
}

impl Iterator for SankakuBetaGallery {
    type Item = crate::Result<GalleryItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_complete {
            return None;
        }

        if self.queue.is_empty() {
            match self.retrieve_batch() {
                Ok(0) => {
                    self.is_complete = true;
                    return None;
                }
                Ok(count) => self.count += count,
                Err(e) => return Some(Err(e)),
            }
        }

        let image = self.queue.pop_front()?;
        match self.client.get(&image.file_url).send() {
            Ok(response) => Some(Ok(GalleryItem::new(image.file_url, response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}

fn build_client() -> crate::Result<Client> {
    use reqwest::header;

    let builder = Client::builder()
        .user_agent(super::USER_AGENT)
        .cookie_store(true);

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/vnd.sankaku.api+json;v=2"),
    );

    Ok(builder.default_headers(headers).build()?)
}
