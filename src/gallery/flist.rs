use std::{collections::HashMap, sync::Arc};

use regex::Regex;
use serde::Deserialize;

use crate::gallery::prelude::*;

pub fn extract(url: &str) -> crate::Result<UnpagedGallery<FlistImage>> {
    // This ought to be basically the easiest implementation yet. My compliments to FList,
    // despite they're not exactly my favorite bunch of people to deal with.

    #[derive(Debug, Deserialize)]
    struct Template {
        #[serde(rename = "images")]
        profile: Vec<Image>,
        error: String,
    }

    let context = Context::new();
    let page_content = context.client.get(url).send()?.text()?;
    let character_id = read_character_id(url, &page_content)?;
    let inline = read_inlines(&page_content).unwrap_or_default();
    let links = read_links(&page_content).unwrap_or_default();

    let Template { profile, .. } = context
        .client
        .post("https://www.f-list.net/json/profile-images.json")
        .form(&[("character_id", character_id)])
        .send()?
        .json()?;

    let mut items = VecDeque::with_capacity(inline.len() + links.len() + profile.len());
    items.extend(inline.into_iter().map(FlistImage::Inline));
    items.extend(profile.into_iter().map(FlistImage::Profile));
    items.extend(links.into_iter().map(FlistImage::Link));

    Ok(UnpagedGallery { context, items })
}

pub enum FlistImage {
    Inline(Inline),
    Profile(Image),
    Link(String),
}

impl Downloadable for FlistImage {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let response = match self {
            FlistImage::Inline(inline) => {
                let d = &inline.hash[..2];
                let e = &inline.hash[2..4];
                let url = format!(
                    "https://static.f-list.net/images/charinline/{}/{}/{}.{}",
                    d, e, inline.hash, inline.extension
                );
                context.client.get(&url).send()?
            }
            FlistImage::Profile(image) => {
                let url = format!(
                    "https://static.f-list.net/images/charimage/{}.{}",
                    image.id, image.extension
                );
                context.client.get(&url).send()?
            }
            FlistImage::Link(url) => context.client.get(&url).send()?,
        };

        Ok(ResponseGalleryItem::new(response))
    }
}

pub struct Context {
    client: Client,
}

impl Context {
    fn new() -> Self {
        use reqwest::{
            cookie::Jar,
            header::{HeaderMap, HeaderValue, ACCEPT},
        };

        let cookie_store = Jar::default();
        let url = "http://www.f-list.net".parse().unwrap();
        cookie_store.add_cookie_str("warning=1", &url);

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .cookie_provider(Arc::new(cookie_store))
                .default_headers(headers)
                .build()
                .unwrap(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Image {
    #[serde(rename = "image_id")]
    id: String,
    extension: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Inline {
    hash: String,
    extension: String,
}

fn read_character_id<'a>(url: &str, content: &'a str) -> crate::Result<&'a str> {
    let pattern =
        Regex::new(r#"<input type='hidden' id='profile-character-id' value='(\d+)'/>"#).unwrap();

    Ok(pattern
        .captures(content)
        .ok_or_else(|| Error::Extraction(ExtractionFailure::Metadata, url.into()))?
        .get(1)
        .unwrap()
        .as_str())
}

fn read_inlines(content: &str) -> Option<Vec<Inline>> {
    let pattern = Regex::new(r#"FList\.Inlines\.inlines ?= ?(\{.+\})"#).unwrap();
    let captures = pattern.captures(content)?;
    let inlines: HashMap<&str, Inline> = serde_json::from_str(captures.get(1)?.as_str()).ok()?;
    Some(inlines.into_iter().map(|(_, v)| v).collect())
}

fn read_links(content: &str) -> Option<Vec<String>> {
    let pattern = Regex::new(r"\[url=([^\]]+static.f-list.net/images/charimage/[^\]]+)\]").unwrap();
    Some(
        pattern
            .captures_iter(content)
            .map(|captures| captures.get(1).unwrap().as_str().into())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn inline_extractor_works() {
        let content = include_str!("../../resource/flist/gallery-page.html");
        let inline = super::read_inlines(content).unwrap();
        assert_eq!(1, inline.len());
    }
}
