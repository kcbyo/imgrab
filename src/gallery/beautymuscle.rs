use nipper::Document;
use regex::Regex;

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<(PagedGallery<BmPager>, Option<String>)> {
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

    // Before we return, it's necessary for us to grab the very first page
    // and take 1) the page count and 2) the first set of results, because
    // otherwise there's no real way to know when this pager needs to stop.

    let context = Context::new();
    let name_from_query = query.name_from_query().map(|s| s.to_string());
    let mut pager = BmPager::new(query);

    let text = context.client.get(&pager.next_url()).send()?.text()?;
    let pattern = Regex::new(r"Page \d+ of (\d+)").unwrap();
    let count = pattern
        .captures(&text)
        .and_then(|x| x.get(1).map(|x| x.as_str()))
        .and_then(|x| x.parse::<usize>().ok())
        .ok_or_else(|| {
            Error::Extraction(
                ExtractionFailure::Metadata,
                String::from("failed to retrieve page count"),
            )
        })?;

    pager.set_max_page(count);

    let gallery = PagedGallery {
        current: context.read_thumbs(&text),
        context,
        pager,
    };

    Ok((gallery, name_from_query))
}

enum Query {
    Tag(String),
    Search(String),
}

impl Query {
    fn name_from_query(&self) -> Option<&str> {
        if let Query::Tag(tag) = &self {
            Some(tag)
        } else {
            None
        }
    }
}

pub struct BmPager {
    query: Query,
    page: usize,
    max_page: Option<usize>,
}

impl BmPager {
    fn new(query: Query) -> Self {
        Self {
            query,
            page: 0,
            max_page: None,
        }
    }

    fn set_max_page(&mut self, max_page: usize) {
        self.max_page = Some(max_page);
    }

    fn next_url(&mut self) -> String {
        self.page += 1;
        match &self.query {
            Query::Tag(tag) => match self.page {
                1 => format!("https://www.beautymuscle.net/pin/tag/{}/", tag),
                n => format!("https://www.beautymuscle.net/pin/tag/{}/page/{}/", tag, n),
            },
            Query::Search(search) => match self.page {
                1 => format!("https://www.beautymuscle.net/?s={}&q=", search),
                n => format!("https://www.beautymuscle.net/page/{}/?s={}&q", n, search),
            },
        }
    }
}

impl Pager for BmPager {
    type Context = Context;

    type Item = Url;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        // This sure would be easier if they'd go on and stabilize Option::contains()
        if self
            .max_page
            .as_ref()
            .map(|&max_page| max_page == self.page)
            .unwrap_or_default()
        {
            return Ok(Page::Empty);
        }

        let text = context.client.get(&self.next_url()).send()?.text()?;
        Ok(context.read_thumbs(&text))
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

    fn read_thumbs(&self, text: &str) -> Page<Url> {
        let document = Document::from(text);
        document
            .select("img.featured-thumb")
            .iter()
            .filter_map(|element| {
                element.attr("src").map(|src| {
                    if let Some(size_match) = self
                        .thumbnail_size_pattern
                        .captures(&*src)
                        .and_then(|x| x.get(1))
                    {
                        let left = &src[..size_match.start()];
                        let right = &src[size_match.end()..];
                        Url(left.to_owned() + right)
                    } else {
                        Url(src.to_string())
                    }
                })
            })
            .collect()
    }
}

/// represents the actual image url
#[derive(Debug)]
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
