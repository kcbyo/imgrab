use regex::Regex;
use serde::Deserialize;

use super::prelude::*;

pub struct Context {
    client: Client,
    gallery_page_pattern: Regex,
    image_metadata_pattern: Regex,
}

impl Context {
    fn new() -> Self {
        Self {
            client: build_client(),
            gallery_page_pattern: Regex::new(r#"index\.php\?page=post&s=view&id=(\d+)"#).unwrap(),
            image_metadata_pattern: Regex::new(r#"image = (\{.+\})"#).unwrap(),
        }
    }

    fn get_gallery_page_links(&self, text: &str) -> Page<ImageId> {
        self.gallery_page_pattern
            .captures_iter(text)
            .filter_map(|cx| cx.get(1))
            .map(|cx| ImageId(cx.as_str().into()))
            .collect()
    }

    fn get_image_metadata(&self, text: &str) -> crate::Result<ImageMetadata> {
        let packet = self
            .image_metadata_pattern
            .captures(text)
            .and_then(|cx| cx.get(1))
            .map(|cx| cx.as_str().replace('\'', "\""))
            .ok_or_else(|| {
                Error::Extraction(
                    ExtractionFailure::ImageUrl,
                    String::from("unable to extract image metadata"),
                )
            })?;

        serde_json::from_str(&packet)
            .map_err(|e| Error::Extraction(ExtractionFailure::Metadata, e.to_string()))
    }
}

pub struct ImageId(String);

impl ImageId {
    fn url(&self) -> String {
        format!(
            "https://rule34.xxx/index.php?page=post&s=view&id={}",
            self.0
        )
    }
}

impl Downloadable for ImageId {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        // 1. Grab gallery page.
        // 2. Get image metadata
        // 3. Generate download url.
        // 4. Profit.

        let text = context.client.get(&self.url()).send()?.text()?;
        let meta = context.get_image_metadata(&text)?;

        Ok(context
            .client
            .get(&meta.url())
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

/// Image metadata extracted from each image page
///
/// Note that this struct omits several members describing the sample image.
#[derive(Clone, Debug, Deserialize)]
pub struct ImageMetadata {
    domain: String,
    dir: i32,
    img: String,
    base_dir: String,
}

impl ImageMetadata {
    fn url(&self) -> String {
        format!("{}{}/{}/{}", self.domain, self.base_dir, self.dir, self.img)
    }
}

pub fn extract(url: &str) -> crate::Result<PagedGallery<Rule34Pager>> {
    let search = extract_search(url)?;
    let pager = Rule34Pager { search, idx: 0 };

    Ok(PagedGallery {
        context: Context::new(),
        pager: pager,
        current: Page::Empty,
    })
}

pub struct Rule34Pager {
    search: String,
    idx: usize,
}

impl Rule34Pager {
    fn get_url(&self) -> String {
        match self.idx {
            // We don't want to give away that we're doing this via automation
            0 => format!(
                "https://rule34.xxx/index.php?page=post&s=list&tags={}",
                self.search
            ),
            n => format!(
                "https://rule34.xxx/index.php?page=post&s=list&tags={}&pid={}",
                self.search, n
            ),
        }
    }
}

impl Pager for Rule34Pager {
    type Context = Context;

    type Item = ImageId;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        let text = context.client.get(&self.get_url()).send()?.text()?;
        Ok(context.get_gallery_page_links(&text))
    }
}

fn extract_search(url: &str) -> crate::Result<String> {
    // https://rule34.xxx/index.php?page=post&s=list&tags=krysdecker&pid=42
    let pattern = Regex::new(r#"tags=([^&#]+)"#).unwrap();
    pattern
        .captures(url)
        .and_then(|cx| cx.get(1))
        .map(|cx| cx.as_str().to_owned())
        .ok_or_else(|| {
            Error::Unsupported(
                UnsupportedError::Route,
                String::from("Rule34 urls must have one or more tags"),
            )
        })
}

fn build_client() -> Client {
    use std::sync::Arc;

    use reqwest::{cookie::Jar, Url};

    let url: Url = "rule34.xxx".parse().unwrap();
    let jar = Jar::default();
    jar.add_cookie_str("gdpr=1", &url);
    jar.add_cookie_str("gdpr-disable-ga=1", &url);
    jar.add_cookie_str("resize-notification=1", &url);
    jar.add_cookie_str("resize-original=1", &url);

    Client::builder()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .cookie_provider(Arc::new(jar))
        .build()
        .unwrap()
}
