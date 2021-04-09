use std::io::Write;

pub mod ehentai;
pub mod fitnakedgirls;
pub mod flist;
pub mod gelbooru;
pub mod girlswithmuscle;
pub mod hentai_foundry;
pub mod imgur;
pub mod nhentai;
pub mod nsfwalbum;
pub mod rule34;
pub mod sankakubeta;

use crate::storage::NameContext;

pub static USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:87.0) Gecko/20100101 Firefox/87.0";

pub trait Gallery {
    fn next(&mut self) -> Option<crate::Result<GalleryItem>>;
    fn advance_by(&mut self, n: usize) -> crate::Result<()>;
}

/// An item parsed from a gallery
///
/// A gallery item may have a name distinct from its url. An example might
/// be a gallery wherein items are numbered 001 through 089 but with useless
/// urls, e.g. /picture/3545ugF8sCX33985. In that case, an implementation
/// may choose to pass on the more useful name along with the url.
pub struct GalleryItem {
    url: String,
    response: reqwest::blocking::Response,
    name: Option<String>,
}

impl GalleryItem {
    pub fn new(url: impl Into<String>, response: reqwest::blocking::Response) -> Self {
        GalleryItem {
            url: url.into(),
            response,
            name: None,
        }
    }

    pub fn with_name<T, U>(url: T, name: U, response: reqwest::blocking::Response) -> Self
    where
        T: Into<String>,
        U: Into<String>,
    {
        GalleryItem {
            url: url.into(),
            response,
            name: Some(name.into()),
        }
    }

    pub fn context(&self) -> NameContext {
        use std::borrow::Cow;

        let content_disposition = self
            .response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION);

        let name = self
            .name
            .as_ref()
            .map(Cow::from)
            .or_else(|| content_disposition.and_then(read_filename).map(Cow::from));

        NameContext::new(&self.url, name)
    }

    pub fn write(&mut self, mut target: impl Write) -> crate::Result<u64> {
        Ok(self.response.copy_to(&mut target)?)
    }
}

fn read_filename(disposition: &reqwest::header::HeaderValue) -> Option<String> {
    // "content-disposition": "attachment; filename=114_Turtlechan_312677_FISHOOKERS_PAGE_3.png"
    let disposition = disposition.to_str().ok()?;
    disposition
        .rfind("filename=")
        .map(|idx| disposition[(idx + 9)..].to_owned())
}

mod prelude {
    pub use crate::{
        error::{Error, ExtractionFailure, UnsupportedError},
        gallery::{Gallery, GalleryItem},
    };
    pub use regex::Regex;
    pub use reqwest::blocking::{Client, Request, Response};
    pub use std::collections::VecDeque;
    pub use std::iter::Skip;
}
