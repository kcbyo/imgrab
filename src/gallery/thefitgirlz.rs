use std::{collections::VecDeque, io};

use nipper::Document;
use regex::Regex;
use ureq::{Agent, AgentBuilder, Response};

use crate::{
    error::{Error, UnsupportedError},
    storage::NameContext,
};

use super::{Downloadable, GalleryItem, PagedGallery, Pager};

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
        agent: AgentBuilder::new().user_agent(super::USER_AGENT).build(),
        pager: FgPager {
            is_complete: false,
            offset: 0,
            model,
        },
        current_page: VecDeque::new(),
    })
}

pub struct Item {
    response: Response,
}

impl GalleryItem for Item {
    fn context(&self) -> crate::storage::NameContext {
        NameContext::from_response(&self.response)
    }

    fn write<W: std::io::Write + ?Sized>(self, writer: &mut W) -> std::io::Result<u64> {
        io::copy(&mut self.response.into_reader(), writer)
    }
}

pub struct FgPager {
    is_complete: bool,
    offset: usize,
    model: String,
}

impl Pager for FgPager {
    type Item = Url;

    fn next_page(
        &mut self,
        agent: &ureq::Agent,
    ) -> Option<crate::Result<std::collections::VecDeque<Self::Item>>> {
        if self.is_complete {
            return None;
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

        let content = match download_page(&url, agent) {
            Ok(content) => content,
            Err(e) => return Some(Err(e)),
        };

        let document = Document::from(&content);
        let links = document
            .select("a.gallery-img-link")
            .iter()
            .filter_map(|entity| entity.attr("href").map(|attr| Url(attr.to_string())));

        Some(Ok(links.collect()))
    }
}

// FIXME: it is not clear to me that this implementation will be correct with
// regard to the end of paged results. That is, when you hit page 7 of 6, will
// this throw an error, or will it send back a response devoid of links?
// Option 2 is fine. Option 1 not so much.
fn download_page(url: &str, agent: &Agent) -> crate::Result<String> {
    Ok(agent.get(url).call()?.into_string()?)
}

pub struct Url(String);

impl AsRef<str> for Url {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Downloadable for Url {
    type Output = Item;

    fn download(self, agent: &ureq::Agent) -> Result<Self::Output, ureq::Error> {
        // Two-step download process:
        // 1. Gallery page
        // 2. Image

        // FIXME: let's sleep on this and think of a way to provide a shared
        // extractor here, like a regex or whatever. That could be a download
        // context (instead of just a bare agent) or... whatever.

        let content = agent.get(&self.0).call()?.into_string()?;

        agent.get(&self.0).call().map(|response| Item { response })
    }
}
