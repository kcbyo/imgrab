use nipper::{Document, Matcher};
use regex::Regex;

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<PagedGallery<FgPager>> {
    let pattern = Regex::new("thefitgirlz.com/gallery/([^/]+)/").unwrap();
    let model = pattern
        .captures(url)
        .ok_or_else(|| {
            Error::Unsupported(
                UnsupportedError::Route,
                String::from("unable to get model name from url"),
            )
        })?
        .get(1)
        .unwrap()
        .as_str()
        .to_owned();

    Ok(PagedGallery {
        context: Context {
            client: Client::builder().user_agent(USER_AGENT).build().unwrap(),
            image_meta_selector: Matcher::new("meta").unwrap(),
            image_name_pattern: Regex::new(r"wp-content/uploads/(\d+)/(\d+)/(.+)").unwrap(),
        },
        pager: FgPager {
            is_complete: false,
            offset: 0,
            model,
        },
        current: Page::Empty,
    })
}

pub struct Context {
    client: Client,
    image_meta_selector: Matcher,
    image_name_pattern: Regex,
}

pub struct FgPager {
    is_complete: bool,
    offset: usize,
    model: String,
}

impl Pager for FgPager {
    type Context = Context;
    type Item = Url;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        if self.is_complete {
            return Ok(Page::Empty);
        }

        // https://thefitgirlz.com/gallery/valentina-lequeux/
        // https://thefitgirlz.com/gallery/valentina-lequeux/page/2/
        let url = match self.offset {
            0 => format!("https://thefitgirlz.com/gallery/{}/", self.model),
            n => format!(
                "https://thefitgirlz.com/gallery/{}/page/{}/",
                self.model,
                n + 1
            ),
        };
        self.offset += 1;

        let content = download_page(&url, &context.client)?;
        let document = Document::from(&content);
        let links = document
            .select("a.gallery-img-link")
            .iter()
            .filter_map(|entity| entity.attr("href").map(|attr| Url(attr.to_string())));

        Ok(links.collect())
    }
}

// FIXME: it is not clear to me that this implementation will be correct with
// regard to the end of paged results. That is, when you hit page 7 of 6, will
// this throw an error, or will it send back a response devoid of links?
// Option 2 is fine. Option 1 not so much.
fn download_page(url: &str, client: &Client) -> crate::Result<String> {
    Ok(client.get(url).send()?.text()?)
}

pub struct Url(String);

impl Downloadable for Url {
    type Context = Context;
    type Output = NamedGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        // Step one: get the image url from the gallery page
        let content = context.client.get(&self.0).send()?.text()?;
        let document = Document::from(&content);
        let url = document
            .select_matcher(&context.image_meta_selector)
            .iter()
            .filter_map(|element| {
                element
                    .attr("property")
                    .map(|attr| "og:image" == &*attr)
                    .unwrap_or_default()
                    .then(|| element.attr("content"))
                    .flatten()
            })
            .next()
            .ok_or_else(|| {
                Error::Extraction(
                    ExtractionFailure::ImageUrl,
                    String::from("image url not found in metadata"),
                )
            })?;

        // Step two: create a new image name based on the image url, because
        // there's way too much repetition in the standard names for these.
        let captures = context.image_name_pattern.captures(&*url).ok_or_else(|| {
            Error::Extraction(
                ExtractionFailure::Metadata,
                format!(
                    "unable to get wp-content upload year/month from text: {}",
                    url
                ),
            )
        })?;

        // Should be impossible to match this pattern without all three groups, so....
        let year = captures.get(1).unwrap().as_str();
        let month = captures.get(2).unwrap().as_str();
        let file = captures.get(3).unwrap().as_str();

        let response = context.client.get(&*url).send()?;
        Ok(NamedGalleryItem::new(
            response,
            format!("{}-{}-{}", year, month, file),
        ))
    }
}
