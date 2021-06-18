use nipper::Document;
use regex::Regex;

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<PagedGallery<BmPager>> {
    let pattern = Regex::new(r"/pin/tag/([^/]+)/?|\?s=([^&]+)").unwrap();
    let captures = pattern.captures(url).ok_or_else(|| {
        Error::Unsupported(
            UnsupportedError::Route,
            String::from("unable to extract gallery information from url"),
        )
    })?;

    let query = match (captures.get(1), captures.get(2)) {
        (Some(tag), None) => Query::Tag(tag.as_str().into()),
        (None, Some(search)) => Query::Search(search.as_str().into()),
        _ => unreachable!("at least one match arm required"),
    };

    Ok(PagedGallery {
        context: Context::new(),
        pager: BmPager::new(query),
        current: Page::Empty,
    })
}

enum Query {
    Tag(String),
    Search(String),
}

pub struct BmPager {
    query: Query,
    page: usize,
    is_complete: bool,
}

impl BmPager {
    fn new(query: Query) -> Self {
        Self {
            query,
            page: 0,
            is_complete: false,
        }
    }

    fn next_url(&mut self) -> String {
        self.page += 1;
        match &self.query {
            Query::Tag(tag) => match self.page {
                0 => format!("https://www.beautymuscle.net/pin/tag/{}/", tag),
                n => format!("https://www.beautymuscle.net/pin/tag/{}/page/{}/", tag, n),
            },
            Query::Search(search) => match self.page {
                0 => format!("https://www.beautymuscle.net/?s={}&q=", search),
                n => format!("https://www.beautymuscle.net/page/{}/?s={}&q", n, search),
            },
        }
    }
}

impl Pager for BmPager {
    type Context = Context;

    type Item = Url;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        if self.is_complete {
            return Ok(Page::Empty);
        }

        let content = context.load_content(&self.next_url())?;
        if content.is_empty() {
            self.is_complete = true;
            return Ok(Page::Empty);
        }

        let document = Document::from(&content);
        let thumbs = document
            .select("a.featured-thumb")
            .iter()
            .filter_map(|element| {
                element
                    .attr("src")
                    .map(|src| Url(context.thumbnail_size_pattern.replace(&*src, "").into()))
            });

        Ok(thumbs.collect())
    }
}

pub struct Context {
    client: Client,
    thumbnail_size_pattern: Regex,
}

impl Context {
    fn new() -> Self {
        Self {
            client: Client::builder().user_agent(USER_AGENT).build().unwrap(),
            thumbnail_size_pattern: Regex::new(r"(-\d+x\d+)\.").unwrap(),
        }
    }

    // FIXME: this has never been tested so... whatever, ok?
    fn load_content(&self, url: &str) -> crate::Result<String> {
        let response = self.client.get(url).send()?;

        // A 301 means we've run out of pages
        if response.status() == 301 {
            Ok(String::new())
        } else {
            Ok(response.text()?)
        }
    }
}

/// represents the actual image url
pub struct Url(String);

impl Downloadable for Url {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(ResponseGalleryItem::new(
            context.client.get(&self.0).send()?,
        ))
    }
}
