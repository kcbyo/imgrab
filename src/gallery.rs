use std::io::Write;

mod ehentai;
mod fitnakedgirls;
mod flist;
mod gelbooru;
mod girlswithmuscle;
mod hentai_foundry;
mod imgur;
mod nhentai;
mod nsfwalbum;
mod rule34;
mod sankaku;
mod sankakubeta;

use crate::storage::NameContext;

pub use self::{
    ehentai::EHentai,
    fitnakedgirls::FitNakedGirls,
    flist::FList,
    gelbooru::Gelbooru,
    girlswithmuscle::GirlsWithMuscle,
    hentai_foundry::HentaiFoundry,
    imgur::{ImgurAlbum, ImgurGallery, ImgurSingle},
    nhentai::NHentai,
    nsfwalbum::NsfwAlbum,
    rule34::Rule34,
    sankaku::Sankaku,
    sankakubeta::SankakuBeta,
};

pub static USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:87.0) Gecko/20100101 Firefox/87.0";

pub type DynamicGallery = Box<(dyn Gallery + 'static)>;

pub trait ReadGallery: Sized {
    fn read(self, url: &str) -> crate::Result<DynamicGallery>;
}

pub trait Gallery: Iterator<Item = crate::Result<GalleryItem>> {
    /// Applies a skip value to the underlying gallery.
    ///
    /// A skip offset MUST be applied without iterating the underlyinggallery, because
    // doing so incurs network costs and may trigger throttling behaviors from the gallery
    // host.
    fn apply_skip(&mut self, skip: usize) -> crate::Result<()>;
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
        gallery::{DynamicGallery, Gallery, GalleryItem, ReadGallery},
    };
    pub use regex::Regex;
    pub use reqwest::blocking::{Client, Request, Response};
    pub use std::collections::VecDeque;
    pub use std::iter::Skip;
}
