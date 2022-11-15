use serde::Deserialize;

use super::prelude::*;

static AJAX_BASE_ROUTE: &str = "https://bitchesgirls.com/ajax/modelpage/";

pub fn extract(url: &str) -> crate::Result<(PagedGallery<BitchesPager>, Option<String>)> {
    let client = build_client(url);
    let route = url
        .strip_prefix("https://bitchesgirls.com/")
        .ok_or_else(|| Error::Unsupported(UnsupportedError::Route, format!("bad url: {url}")))?;

    let init_url = format!("{AJAX_BASE_ROUTE}{route}1/");
    let init_response: AlbumResponse = client.get(&init_url).send()?.json()?;
    let pager = BitchesPager::new(route, init_response.pages_amount);

    Ok((
        PagedGallery {
            context: Context {
                client,
                cdn: init_response.cdn_name,
            },
            current: init_response
                .album
                .content
                .into_iter()
                .map(|content| Item(content.original))
                .collect(),
            pager,
        },
        Some(init_response.album.album_id),
    ))
}

pub struct Context {
    client: Client,
    cdn: String,
}

impl Context {
    fn cdn_url(&self, file: &str) -> String {
        // I don't think there are any matches at this point, but I'm fed up
        // with this particular bug, so....
        self.cdn.trim_end_matches('/').to_string() + "/file/" + file.trim_start_matches('/')
    }
}

pub struct BitchesPager {
    route: String,
    pages: Box<dyn Iterator<Item = usize>>,
}

impl BitchesPager {
    fn new(route: &str, count: i64) -> Self {
        Self {
            route: route.into(),
            pages: Box::new((1..=count).skip(1).map(|n| n as usize)),
        }
    }
}

impl Pager for BitchesPager {
    type Context = Context;

    type Item = Item;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        let url = match self.pages.next() {
            Some(page) => format!("{AJAX_BASE_ROUTE}{}{page}/", self.route),
            None => return Ok(Page::Empty),
        };

        let response: AlbumResponse = context.client.get(&url).send()?.json()?;

        Ok(response
            .album
            .content
            .into_iter()
            .map(|content| Item(content.original))
            .collect())
    }
}

pub struct Item(String);

impl Downloadable for Item {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let url = context.cdn_url(&self.0);
        Ok(context
            .client
            .get(&url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

#[derive(Deserialize)]
pub struct AlbumResponse {
    album: Album,
    #[serde(rename = "pagesAmount")]
    pages_amount: i64,
    #[serde(rename = "cdnName")]
    cdn_name: String,
}

#[derive(Deserialize)]
pub struct Album {
    album_id: String,
    content: Vec<Content>,
}

#[derive(Deserialize)]
pub struct Content {
    original: String,
}

fn build_client(referer: &str) -> Client {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, REFERER};

    let mut headers = HeaderMap::new();

    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(REFERER, HeaderValue::from_str(referer).unwrap());

    Client::builder()
        .user_agent(USER_AGENT)
        .referer(false)
        .default_headers(headers)
        .build()
        .unwrap()
}
