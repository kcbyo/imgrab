use std::borrow::Cow;

use regex::Regex;
use scraper::{Html, Selector};
use serde::Deserialize;

use super::{prelude::*, Gallery};

pub fn extract(url: &str) -> crate::Result<(Rule34Gallery, Option<String>)> {
    let search = extract_search(url)?;
    let pager = Rule34Pager { search, idx: 0 };

    let gallery = Rule34Gallery {
        context: Context::new(),
        pager,
        current: Page::Empty,
    };

    match get_single_tag(&gallery.pager.search).map(|tag| tag.to_owned()) {
        Some(tag) => Ok((gallery, Some(tag))),
        None => Ok((gallery, None)),
    }
}

pub struct Context {
    client: Client,
    gallery_page_pattern: Regex,
    image_metadata_pattern: Regex,
    video_container_pattern: Regex,
    video_selector: Selector,
}

impl Context {
    fn new() -> Self {
        Self {
            client: build_client(),
            gallery_page_pattern: Regex::new(r#"index\.php\?page=post&s=view&id=(\d+)"#).unwrap(),
            image_metadata_pattern: Regex::new(r#"image = (\{.+\})"#).unwrap(),

            video_container_pattern: Regex::new("gelcomVideoContainer").unwrap(),
            video_selector: Selector::parse("div#gelcomVideoContainer > video > source").unwrap(),
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
        // Rule34 presents both images and video. Video is treated similarly to images, but with
        // the addition of a "gelcomVideoPlayer" element. The resource url presented in the usual
        // image payload, probably as the result of a bug, is not the correct url for the video,
        // which is given in the video player element. To unfuck this...

        // Fuck it.

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

        let mut metadata: ImageMetadata = serde_json::from_str(&packet)
            .map_err(|e| Error::Extraction(ExtractionFailure::Metadata, e.to_string()))?;

        if self.video_container_pattern.is_match(text) {
            let document = Html::parse_document(text);
            let container_src = document
                .select(&self.video_selector)
                .next()
                .and_then(|cx| cx.value().attr("src"))
                .ok_or_else(|| {
                    Error::Extraction(
                        ExtractionFailure::ImageUrl,
                        String::from("unable to extract video metadata"),
                    )
                })?;

            metadata.video_url = Some(container_src.into());
        }

        Ok(metadata)
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

        let text = context.client.get(self.url()).send()?.text()?;
        let meta = context.get_image_metadata(&text)?;
        let url = meta.url();

        Ok(context
            .client
            .get(&*url)
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

    #[serde(skip_deserializing)]
    video_url: Option<String>,
}

impl ImageMetadata {
    fn url(&self) -> Cow<str> {
        self.video_url
            .as_deref()
            .map(|s| s.into())
            .unwrap_or_else(|| {
                format!("{}{}/{}/{}", self.domain, self.base_dir, self.dir, self.img).into()
            })
    }
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
        let text = context.client.get(self.get_url()).send()?.text()?;
        let links = context.get_gallery_page_links(&text);
        self.idx += links.len();
        Ok(context.get_gallery_page_links(&text))
    }
}

// This specialized gallery impl exists solely to provide specialized
// skip behavior for rule34. Sure wish the rust crew would go on and
// merge specialization....
pub struct Rule34Gallery {
    context: Context,
    pager: Rule34Pager,
    current: Page<ImageId>,
}

impl Gallery for Rule34Gallery {
    type Item = ResponseGalleryItem;

    // Copy/paste from standard gallery implementation
    fn next(&mut self) -> Option<crate::Result<Self::Item>> {
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
        const RULE34_PAGE_SIZE: usize = 42; // NEVER CHANGE, GUYS!

        let mut skipped = 0;
        let mut skip_remaining = n;

        let advance_pages = n / RULE34_PAGE_SIZE;
        if advance_pages > 0 {
            skipped = advance_pages * RULE34_PAGE_SIZE;
            skip_remaining -= skipped;
            self.pager.idx += skipped;
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

    let url: Url = "https://rule34.xxx".parse().unwrap();
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

fn get_single_tag(tags: &str) -> Option<&str> {
    let mut tags = tags.trim_matches('+').split('+');
    tags.next().filter(|_| tags.next().is_none())
}
