use std::collections::HashMap;

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
    let page_content = context.get(url)?.into_string()?;
    let character_id = read_character_id(url, &page_content)?;
    let inline = read_inlines(&page_content).unwrap_or_default();
    let links = read_links(&page_content).unwrap_or_default();

    let Template { profile, .. } = context
        .configure_post("https://www.f-list.net/json/profile-images.json")
        .send_form(&[("character_id", character_id)])?
        .into_json()?;

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

    type Output = UreqGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let response = match self {
            FlistImage::Inline(inline) => {
                let d = &inline.hash[..2];
                let e = &inline.hash[2..4];
                let url = format!(
                    "https://static.f-list.net/images/charinline/{}/{}/{}.{}",
                    d, e, inline.hash, inline.extension
                );
                context.get(&url)?
            }
            FlistImage::Profile(image) => {
                let url = format!(
                    "https://static.f-list.net/images/charimage/{}.{}",
                    image.id, image.extension
                );
                context.get(&url)?
            }
            FlistImage::Link(url) => context.get(&url)?,
        };

        Ok(UreqGalleryItem::new(response))
    }
}

pub struct Context {
    agent: Agent,
}

impl Context {
    fn new() -> Self {
        Self {
            agent: AgentBuilder::new().user_agent(USER_AGENT).build(),
        }
    }

    fn get(&self, url: &str) -> Result<ureq::Response, ureq::Error> {
        self.agent
            .get(url)
            .set("Accept", "application/json")
            .set("Cookie", "warning=1")
            .call()
    }

    fn configure_post(&self, url: &str) -> ureq::Request {
        self.agent
            .post(url)
            .set("Accept", "application/json")
            .set("Cookie", "warning=1")
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
