use std::collections::{HashMap, VecDeque};

use regex::Regex;
use serde::Deserialize;

use crate::gallery::prelude::*;

pub struct FList;

impl ReadGallery for FList {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
        // This ought to be basically the easiest implementation yet. My compliments to FList,
        // despite they're not exactly my favorite bunch of people to deal with.

        #[derive(Debug, Deserialize)]
        struct Template {
            #[serde(rename = "images")]
            profile: VecDeque<Image>,
            error: String,
        }

        let client = build_client()?;
        let page_content = client.get(url).send()?.text()?;
        let character_id = read_character_id(url, &page_content)?;
        let inline = read_inlines(&page_content).unwrap_or_default();
        let Template { profile, .. } = client
            .post("https://www.f-list.net/json/profile-images.json")
            .form(&[("character_id", character_id)])
            .send()?
            .json()?;

        Ok(Box::new(FListGallery {
            client,
            inline,
            profile,
        }))
    }
}

pub struct FListGallery {
    client: Client,
    inline: VecDeque<Inline>,
    profile: VecDeque<Image>,
}

impl FListGallery {
    fn next_inline(&mut self) -> Option<String> {
        self.inline.pop_front().map(|inline| {
            let d = &inline.hash[..2];
            let e = &inline.hash[2..4];
            format!(
                "https://static.f-list.net/images/charinline/{}/{}/{}.{}",
                d, e, inline.hash, inline.extension
            )
        })
    }

    fn next_profile(&mut self) -> Option<String> {
        self.profile.pop_front().map(|image| {
            format!(
                "https://static.f-list.net/images/charimage/{}.{}",
                image.id, image.extension
            )
        })
    }
}

impl Gallery for FListGallery {
    fn apply_skip(&mut self, skip: usize) -> crate::Result<()> {
        use std::cmp;
        let remaining = skip
            - self
                .inline
                .drain(..cmp::min(skip, self.inline.len()))
                .count();
        let remaining = cmp::min(remaining, self.profile.len());
        self.profile.drain(..remaining);
        Ok(())
    }
}

impl Iterator for FListGallery {
    type Item = crate::Result<GalleryItem>;
    fn next(&mut self) -> Option<Self::Item> {
        let url = self.next_inline().or_else(|| self.next_profile())?;
        match self.client.get(&url).send() {
            Ok(response) => Some(Ok(GalleryItem::new(url, response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Image {
    #[serde(rename = "image_id")]
    id: String,
    extension: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Inline {
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

fn read_inlines(content: &str) -> Option<VecDeque<Inline>> {
    let pattern = Regex::new(r#"FList\.Inlines\.inlines ?= ?(\{.+\})"#).unwrap();
    let captures = pattern.captures(content)?;
    let inlines: HashMap<&str, Inline> = serde_json::from_str(captures.get(1)?.as_str()).ok()?;
    Some(inlines.into_iter().map(|(_, v)| v).collect())
}

fn build_client() -> crate::Result<Client> {
    use reqwest::header;

    let builder = Client::builder();
    let mut headers = header::HeaderMap::new();

    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(super::USER_AGENT),
    );
    headers.insert(
        header::COOKIE,
        header::HeaderValue::from_static("warning=1"),
    );

    Ok(builder.default_headers(headers).build()?)
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
