use std::collections::{HashMap, VecDeque};

use regex::Regex;

use crate::{
    config::{Configuration, Key},
    gallery::prelude::*,
};

fn configure_client(username: &str, password: &str) -> crate::Result<Client> {
    use serde::Serialize;

    // This struct looks ridiculous, but it represents the form post required to successfully
    // authenticate to e-hentai's back end. God knows what all this crap is for.
    #[derive(Serialize)]
    struct Form<'a> {
        #[serde(rename = "CookieDate")]
        cookie_date: usize,
        b: &'static str,
        bt: &'static str,
        #[serde(rename = "UserName")]
        username: &'a str,
        #[serde(rename = "PassWord")]
        password: &'a str,
        ipb_login_submit: &'static str,
    }

    impl Form<'_> {
        fn new<'a>(username: &'a str, password: &'a str) -> Form<'a> {
            Form {
                cookie_date: 1,
                b: "d",
                bt: "1-1",
                username,
                password,
                ipb_login_submit: "Login!",
            }
        }
    }

    let client = reqwest::blocking::Client::new();
    let response = client
        .post("https://forums.e-hentai.org/index.php?act=Login&CODE=01")
        .form(&Form::new(username, password))
        .send()?;

    let cookies = read_cookies(&response);
    build_client(cookies)
}

pub fn extract(url: &str) -> crate::Result<EHentaiGallery> {
    // So, one ugly fact about the e-hentai implementation is that the only way to get
    // full-sized images from e-hentai is by logging in. I've already figured out (read:
    // implemented in another program) their authentication mechanism, so it's not that
    // big a deal, but I *am* gonna split that out into a different function from the
    // initializer for the gallery itself. The reader will do the login here, then pass
    // the (logged-in) client to the gallery.

    let config = Configuration::init();
    let username = config.get_config(Key::EHentaiUser)?;
    let password = config.get_config(Key::EHentaiPass)?;
    let client = configure_client(username, password)?;

    // Before we begin, we need to grab some gallery metadata: specifically, we need the
    // page size and the total image count for the gallery. While we're at it, we may as
    // well also grab the first batch of images, too.
    let page_content = client.get(url).send()?.text()?;
    let (_page_size, gallery_size) = read_meta(url, &page_content)?;
    let image_page_pattern = Regex::new(r#"https://e-hentai.org/s/[^"]+"#).unwrap();
    let queue: VecDeque<_> = image_page_pattern
        .captures_iter(&page_content)
        .map(|s| s.get(0).unwrap().as_str().into())
        .collect();

    Ok(EHentaiGallery {
        client,
        base_url: url.into(),
        page: 1,
        count: queue.len(),
        queue,
        // page_size,
        gallery_size,
        image_page_pattern,
        image_url_pattern: Regex::new(r#"id="img" src="([^"]+)"#).unwrap(),
        image_url_pattern_fullsize: Regex::new(r#"<a href="([^"]+)">Download original"#).unwrap(),
    })
}

/// An image url in a given size.
///
/// This type is mostly useless, but is used to provide debugging information about the source
/// of images.
#[derive(Debug)]
pub enum SizeOption {
    Thumb(String),
    Full(String),
}

impl SizeOption {
    fn unwrap(self) -> String {
        match self {
            SizeOption::Thumb(s) | SizeOption::Full(s) => s,
        }
    }
}

impl AsRef<str> for SizeOption {
    fn as_ref(&self) -> &str {
        match self {
            SizeOption::Thumb(s) | SizeOption::Full(s) => s.as_ref(),
        }
    }
}

#[derive(Debug)]
pub struct EHentaiGallery {
    client: Client,
    base_url: String,
    page: usize,
    count: usize,
    queue: VecDeque<String>,

    // Page size could be used, hypothetically, to improve the efficiency of the iterator
    // functions skip and take.
    // page_size: usize,
    gallery_size: usize,

    // Patterns
    image_page_pattern: Regex,
    image_url_pattern: Regex,
    image_url_pattern_fullsize: Regex,
}

impl EHentaiGallery {
    fn retrieve_batch(&mut self) -> crate::Result<usize> {
        let url = format!("{}?p={}", self.base_url, self.page);
        let page_content = self.client.get(&url).send()?.text()?;
        self.queue = self.read_batch(&page_content);
        Ok(self.queue.len())
    }

    fn retrieve_image_url(&self, url: &str) -> crate::Result<SizeOption> {
        let page_content = self.client.get(url).send()?.text()?;

        // There are two flavors of image: full size and standard. In the
        // event there is no full-size image, fall back to standard.
        self.image_url_pattern_fullsize
            .captures(&page_content)
            .map(|s| {
                let url = s.get(1).unwrap().as_str().replace("&amp;", "&");
                SizeOption::Full(url)
            })
            .or_else(|| {
                self.image_url_pattern
                    .captures(&page_content)
                    .map(|s| SizeOption::Thumb(s.get(1).unwrap().as_str().into()))
            })
            .ok_or_else(|| Error::Extraction(ExtractionFailure::ImageUrl, url.into()))
    }

    fn read_batch(&self, content: &str) -> VecDeque<String> {
        self.image_page_pattern
            .captures_iter(content)
            .map(|s| s.get(0).unwrap().as_str().into())
            .collect()
    }
}

impl Gallery for EHentaiGallery {
    fn advance_by(&mut self, mut skip: usize) -> crate::Result<()> {
        loop {
            if skip == 0 {
                return Ok(());
            }

            if self.count >= self.gallery_size {
                if skip > self.queue.len() {
                    self.queue.clear();
                } else {
                    self.queue.drain(..skip);
                }
                return Ok(());
            }

            if skip < self.queue.len() {
                self.queue.drain(..skip);
                return Ok(());
            }

            skip = skip.saturating_sub(self.queue.len());
            self.queue.clear();
            self.count += self.retrieve_batch()?;
            self.page += 1;
        }
    }

    fn next(&mut self) -> Option<crate::Result<GalleryItem>> {
        // E-hentai has some peculiarities re: its gallery design that make the way we do things
        // here a little strange. For a start, you'll never be shown an empty gallery page. An
        // attempt to increment your position past the final page of a gallery will result in
        // the final page being displayed again. Because of this, we'll keep track not only of
        // our current page but also of the number of images we've encountered so far.
        //
        // This number should be compared to the total number of images expected, which must also
        // be extracted from the gallery pages themselves, in order to know when we should cease
        // iteration.

        if self.queue.is_empty() {
            if self.count >= self.gallery_size {
                return None;
            }

            match self.retrieve_batch() {
                Ok(count) => {
                    self.count += count;
                    self.page += 1;
                }
                Err(e) => return Some(Err(e)),
            }
        }

        let image_page = self.queue.pop_front()?;
        let image_url = match self.retrieve_image_url(&image_page) {
            Ok(url) => url,
            Err(e) => return Some(Err(e)),
        };

        match self.client.get(image_url.as_ref()).send() {
            Ok(response) => Some(Ok(GalleryItem::new(image_url.unwrap(), response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}

fn read_cookies(response: &Response) -> HashMap<String, String> {
    // "set-cookie": "ipb_session_id=17f5a3fe9fde37b28d9f4584f67705bc; path=/; domain=.e-hentai.org",
    // "set-cookie": "ipb_coppa=0; path=/; domain=forums.e-hentai.org",
    // "set-cookie": "ipb_session_id=fa9c1ddfc3b9f425ad23a77364dc1677; path=/; domain=.e-hentai.org",

    let mut map = HashMap::new();
    let header_pattern = Regex::new(r#"(\w+)=(\w+)"#).unwrap();
    let headers = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .into_iter()
        .filter_map(|header| header.to_str().ok());

    for header in headers {
        if let Some(captures) = header_pattern.captures(header) {
            map.insert(
                captures.get(1).unwrap().as_str().into(),
                captures.get(2).unwrap().as_str().into(),
            );
        }
    }

    map
}

fn build_client(cookies: HashMap<String, String>) -> crate::Result<Client> {
    use reqwest::header;
    use std::fmt::Write;

    let builder = Client::builder();
    let mut headers = header::HeaderMap::new();
    let mut buffer = String::from("nw=1");

    for (key, value) in cookies {
        let _ = write!(buffer, "; {}={}", key, value);
    }

    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("text/html"),
    );
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(super::USER_AGENT),
    );
    headers.insert(
        header::COOKIE,
        header::HeaderValue::from_str(&buffer).expect("Bad header value (cookies)"),
    );

    Ok(builder.default_headers(headers).build()?)
}

fn read_meta(url: &str, content: &str) -> crate::Result<(usize, usize)> {
    let pattern =
        Regex::new(r#"<p class="gpc">Showing (\d+) - (\d+) of (\d+) images</p>"#).unwrap();
    let captures = pattern
        .captures(content)
        .ok_or_else(|| Error::Extraction(ExtractionFailure::Metadata, url.into()))?;

    let page_start = read_num(captures.get(1).unwrap().as_str())?;
    let page_end = read_num(captures.get(2).unwrap().as_str())?;
    let gallery_size = read_num(captures.get(3).unwrap().as_str())?;

    Ok((page_end - page_start + 1, gallery_size))
}

fn read_num(s: &str) -> crate::Result<usize> {
    s.parse().map_err(|e| {
        Error::Other(
            String::from("Unable to parse gallery metadata"),
            Box::new(e),
        )
    })
}
