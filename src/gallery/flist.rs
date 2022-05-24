use std::{collections::HashMap, sync::Arc};

use regex::Regex;
use serde::Deserialize;

use crate::gallery::prelude::*;

pub fn extract(url: &str) -> crate::Result<(UnpagedGallery<FlistImage>, Option<String>)> {
    // This ought to be basically the easiest implementation yet. My compliments to FList,
    // despite they're not exactly my favorite bunch of people to deal with.

    #[derive(Debug, Deserialize)]
    struct Template {
        #[serde(rename = "images")]
        profile: Vec<Image>,
        // error: String,
    }

    // We want to grab the character name for later. To accomplish this, we're going to use a
    // pair of regular expressions. Sue me. It's better to do this work here, where we have
    // some context about what we're looking at, than to try to do it in a super-generic way
    // later on in the program.

    let name_expr = Regex::new(r#"/c/([^?]+)"#).unwrap();
    let percent_encode_expr = Regex::new(r#"%\d+"#).unwrap();

    let gallery_name = name_expr
        .captures(url)
        .and_then(|cx| cx.get(1).map(|m| m.as_str()));
    let gallery_name = gallery_name.map(|name| {
        percent_encode_expr.replace_all(name, |cx: &regex::Captures| {
            cx.get(0)
                .map(|m| match m.as_str() {
                    "%20" => " ",
                    _ => "",
                })
                .unwrap_or("")
        })
    });

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

    Ok((
        UnpagedGallery { context, items },
        gallery_name.map(|name| name.to_string()),
    ))
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
