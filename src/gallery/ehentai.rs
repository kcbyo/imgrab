use std::{collections::HashMap, ops::Not};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::{Configuration, Key};

use super::{prelude::*, Gallery};

pub fn extract(url: &str) -> crate::Result<(EHentaiGallery, Option<String>)> {
    // First thing first: we have to log in to get full-size images.

    let config = Configuration::init();
    let username = config.get_config(Key::EHentaiUser)?;
    let password = config.get_config(Key::EHentaiPass)?;
    let client = configure_client(username, password)?;

    // Next, hit the API for gallery metadata. This metadata is almost completely useless, but it
    // gives us the page count without a lot of rigamarole.

    // This API request consists of a "method" (which never changes, because we only know of one)
    // and a list of gallery identifiers. The identifiers are stored in a heterogenous array,
    // because apparently 2020 ruined everything. Currently, I'm trying to get serde to serialize
    // a tuple as a heterogenous array.

    static API_URL: &str = "https://api.e-hentai.org/api.php";

    #[derive(Serialize)]
    struct ApiRequest {
        method: &'static str,
        gidlist: Vec<(i32, String)>,
    }

    impl ApiRequest {
        fn new(gid: i32, gtoken: impl Into<String>) -> Self {
            Self {
                method: "gdata",
                gidlist: vec![(gid, gtoken.into())],
            }
        }
    }

    #[derive(Deserialize)]
    pub struct ApiResponse {
        gmetadata: Vec<Gmetadatum>,
    }

    #[derive(Deserialize)]
    pub struct Gmetadatum {
        title: String,
        title_jpn: String,
        filecount: String,

        // No clue what we're ever gonna do with this, but I want to keep it around....
        #[allow(unused)]
        filesize: i64,
    }

    impl Gmetadatum {
        fn auto_name(&self) -> Option<String> {
            self.title
                .is_empty()
                .not()
                .then_some(&*self.title)
                .or_else(|| self.title_jpn.is_empty().not().then_some(&*self.title_jpn))
                .map(|x| x.into())
        }
    }

    let url_expr = Regex::new(r#"/g/(\d+)/(\w+)/?"#).unwrap();
    let (gid, gtoken) = url_expr
        .captures(url)
        .and_then(|cx| {
            let gid: i32 = cx.get(1).unwrap().as_str().parse().ok()?;
            Some((gid, cx.get(2).unwrap().as_str()))
        })
        .ok_or_else(|| Error::Extraction(ExtractionFailure::Metadata, url.into()))?;
    let request = ApiRequest::new(gid, gtoken);
    let response: ApiResponse = client.post(API_URL).json(&request).send()?.json()?;
    let meta = &response.gmetadata[0];

    // Are you fucking shitting me?
    let gallery_size: usize = response.gmetadata[0]
        .filecount
        .parse()
        .map_err(|_| Error::Extraction(ExtractionFailure::Metadata, url.into()))?;

    let page_content = client.get(url).send()?.text()?;
    let image_page_pattern = Regex::new(r#"https://e-hentai.org/s/[^"]+"#).unwrap();
    let queue: Page<_> = image_page_pattern
        .captures_iter(&page_content)
        .map(|s| EhentaiUrl(s.get(0).unwrap().as_str().into()))
        .collect();

    let gallery = EHentaiGallery {
        context: Context::with_client(client),
        pager: EhentaiPager {
            base_url: url.into(),
            page: 1,
            paged_count: queue.len(),
            total_count: gallery_size,
        },
        current: queue,
    };

    Ok((gallery, meta.auto_name()))
}

pub struct EhentaiPager {
    base_url: String,
    page: usize,

    // If paged count matches or exceeds total count, we are done.
    paged_count: usize,
    total_count: usize,
}

impl Pager for EhentaiPager {
    type Context = Context;

    type Item = EhentaiUrl;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        // E-hentai has some peculiarities re: its gallery design that make the way we do things
        // here a little strange. For a start, you'll never be shown an empty gallery page. An
        // attempt to increment your position past the final page of a gallery will result in
        // the final page being displayed again. Because of this, we'll keep track not only of
        // our current page but also of the number of images we've encountered so far.
        //
        // This number should be compared to the total number of images expected, which must also
        // be extracted from the gallery pages themselves, in order to know when we should cease
        // iteration.

        if self.paged_count >= self.total_count {
            return Ok(Page::Empty);
        }

        let url = format!("{}?p={}", self.base_url, self.page);
        self.page += 1;
        let text = context.client.get(url).send()?.text()?;
        let page: Page<_> = context
            .page_url_pattern
            .find_iter(&text)
            .map(|x| EhentaiUrl(x.as_str().into()))
            .collect();
        self.paged_count += page.len();
        Ok(page)
    }
}

pub struct Context {
    client: Client,
    page_url_pattern: Regex,
    full_size_pattern: Regex,
    thumbnail_pattern: Regex,
}

impl Context {
    fn with_client(client: Client) -> Self {
        Self {
            client,
            page_url_pattern: Regex::new(r#"https://e-hentai.org/s/[^"]+"#).unwrap(),
            thumbnail_pattern: Regex::new(r#"id="img" src="([^"]+)"#).unwrap(),
            full_size_pattern: Regex::new(r#"<a href="([^"]+)">Download original"#).unwrap(),
        }
    }

    fn retrieve_image_url(&self, url: &str) -> crate::Result<String> {
        // There are two flavors of image: full size and standard. In the
        // event there is no full-size image, fall back to standard.
        let text = self.client.get(url).send()?.text()?;
        self.extract_full_size(&text)
            .or_else(|| self.extract_thumbnail(&text))
            .ok_or_else(|| Error::Extraction(ExtractionFailure::ImageUrl, url.into()))
    }

    fn extract_full_size(&self, text: &str) -> Option<String> {
        self.full_size_pattern
            .captures(text)
            .map(|captures| captures.get(1).unwrap().as_str().replace("&amp;", "&"))
    }

    fn extract_thumbnail(&self, text: &str) -> Option<String> {
        self.thumbnail_pattern
            .captures(text)
            .map(|captures| captures.get(1).unwrap().as_str().into())
    }
}

#[derive(Clone, Debug)]
pub struct EhentaiUrl(String);

impl Downloadable for EhentaiUrl {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let url = context.retrieve_image_url(&self.0)?;
        Ok(context
            .client
            .get(url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

// This specialized gallery impl exists to improve efficiency
// in skipping back pages for ehentai.
pub struct EHentaiGallery {
    context: Context,
    pager: EhentaiPager,
    current: Page<EhentaiUrl>,
}

impl Gallery for EHentaiGallery {
    type Item = ResponseGalleryItem;

    fn next(&mut self) -> Option<crate::Result<Self::Item>> {
        // Copied from PagedGallery implementation;
        // there might be a better way to share code here.
        if self.current.is_empty() {
            self.current = match self.pager.next_page(&self.context) {
                Ok(page) if page.is_empty() => return None,
                Ok(page) => page,
                Err(e) => return Some(Err(e)),
            };
        }

        let item = self.current.pop()?;
        Some(item.download(&self.context))
    }

    fn advance_by(&mut self, n: usize) -> crate::Result<usize> {
        let mut skipped = 0;
        let mut skip_remaining = n;

        // This seems to work for large skip values; it is untested
        // for small ones.
        let advance_pages = n / 40;
        if advance_pages > 0 {
            self.pager.page += advance_pages - 1;
            self.pager.paged_count = advance_pages * 40;
            skipped = advance_pages * 40;
            skip_remaining = n - skipped;
            self.current = self.pager.next_page(&self.context)?;
        }

        // Copied from PagedGallery impl
        loop {
            if self.current.is_empty() {
                self.current = self.pager.next_page(&self.context)?;
            }

            if self.current.len() > skip_remaining {
                self.current.drain(skip_remaining);
                return Ok(skipped + skip_remaining);
            } else {
                skipped += self.current.len();
                skip_remaining -= self.current.len();
                self.current.clear();
            }
        }
    }
}

fn configure_client(username: &str, password: &str) -> crate::Result<Client> {
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

    Ok(build_client(read_cookies(&response)))
}

fn build_client(cookies: HashMap<String, String>) -> Client {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, USER_AGENT};
    use std::fmt::Write;

    let builder = Client::builder();
    let mut headers = HeaderMap::new();
    let mut buffer = String::from("nw=1");

    for (key, value) in cookies {
        let _ = write!(buffer, "; {}={}", key, value);
    }

    headers.insert(ACCEPT, HeaderValue::from_static("text/html"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(super::prelude::USER_AGENT),
    );
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&buffer).expect("Bad header value (cookies)"),
    );

    builder.default_headers(headers).build().unwrap()
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
