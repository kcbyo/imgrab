pub mod atfbooru;
pub mod beautymuscle;
pub mod bitchesgirls;
pub mod bleachbooru;
pub mod ehentai;
pub mod exhentai;
pub mod fappeningbook;
pub mod fitnakedgirls;
pub mod flist;
pub mod fucks_pics;
pub mod gelbooru;
pub mod girlswithmuscle;
pub mod hdporn;
pub mod hentai_foundry;
pub mod imgur;
pub mod nhentai;
pub mod novelcrow;
pub mod nsfwalbum;
pub mod rule34;
pub mod rule34_us;
pub mod sankakubeta;
pub mod scrolller;
pub mod thefitgirlz;

use std::{
    borrow::Cow,
    collections::VecDeque,
    io::{self, Write},
    iter::FromIterator,
};

use reqwest::blocking::Response;

use crate::storage::NameContext;

pub(crate) fn build_client() -> prelude::Client {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("text/html"));
    prelude::Client::builder()
        .user_agent(prelude::USER_AGENT)
        .default_headers(headers)
        .build()
        .unwrap()
}

#[derive(Clone, Debug)]
pub enum Page<T> {
    Items(VecDeque<T>),
    Empty,
}

impl<T> Page<T> {
    fn pop(&mut self) -> Option<T> {
        match self {
            Page::Items(items) => items.pop_front(),
            Page::Empty => None,
        }
    }

    fn len(&self) -> usize {
        match self {
            Page::Items(items) => items.len(),
            Page::Empty => 0,
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Page::Items(items) => items.is_empty(),
            Page::Empty => true,
        }
    }

    fn drain(&mut self, count: usize) {
        if let Page::Items(items) = self {
            let _ = items.drain(..count);
        }
    }

    fn clear(&mut self) {
        *self = Page::Empty;
    }
}

impl<A> FromIterator<A> for Page<A> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let items: VecDeque<_> = iter.into_iter().collect();
        if items.is_empty() {
            Page::Empty
        } else {
            Page::Items(items)
        }
    }
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Page::Empty
    }
}

pub trait Gallery {
    type Item: GalleryItem;

    /// Retrieve the next gallery item for download.
    fn next(&mut self) -> Option<crate::Result<Self::Item>>;

    /// Attempts to advance the gallery by n items.
    ///
    /// Returns the number of items actually skipped.
    fn advance_by(&mut self, n: usize) -> crate::Result<usize>;
}

pub trait GalleryItem {
    fn context(&self) -> NameContext;
    fn write<W: Write + ?Sized>(self, writer: &mut W) -> crate::Result<u64>;
}

/// A basic gallery item implemented on only a bare [`reqwest::blocking::Response`]
pub struct ResponseGalleryItem {
    response: Response,
}

impl ResponseGalleryItem {
    pub fn new(response: Response) -> Self {
        Self { response }
    }
}

impl GalleryItem for ResponseGalleryItem {
    fn context(&self) -> NameContext {
        NameContext::from_response(&self.response)
    }

    fn write<W: io::Write + ?Sized>(mut self, writer: &mut W) -> crate::Result<u64> {
        Ok(self.response.copy_to(writer)?)
    }
}

/// A gallery item with an explicitly-overridden name
pub struct NamedGalleryItem {
    name: String,
    response: Response,
}

impl NamedGalleryItem {
    pub fn new(response: Response, name: impl Into<String>) -> Self {
        Self {
            response,
            name: name.into(),
        }
    }
}

impl GalleryItem for NamedGalleryItem {
    fn context(&self) -> NameContext {
        NameContext::new(self.response.url().as_ref(), Some(Cow::from(&self.name)))
    }

    fn write<W: io::Write + ?Sized>(mut self, writer: &mut W) -> crate::Result<u64> {
        Ok(self.response.copy_to(writer)?)
    }
}

pub trait Downloadable {
    type Context;
    type Output: GalleryItem;

    /// Materialize a downloadable item as a gallery item.
    fn download(self, context: &Self::Context) -> crate::Result<Self::Output>;
}

pub trait Pager {
    type Context;
    type Item: Downloadable<Context = Self::Context>;
    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>>;
}

pub struct UnpagedGallery<T: Downloadable> {
    context: T::Context,
    items: VecDeque<T>,
}

impl<T: Downloadable> Gallery for UnpagedGallery<T> {
    type Item = T::Output;

    fn next(&mut self) -> Option<crate::Result<Self::Item>> {
        let item = self.items.pop_front()?;
        Some(item.download(&self.context))
    }

    fn advance_by(&mut self, n: usize) -> crate::Result<usize> {
        if n < self.items.len() {
            let _ = self.items.drain(..n);
            Ok(n)
        } else {
            let len = self.items.len();
            self.items.clear();
            Ok(len)
        }
    }
}

pub struct PagedGallery<T: Pager> {
    context: T::Context,
    pager: T,
    current: Page<T::Item>,
}

impl<T: Pager> PagedGallery<T> {
    fn default_advance_by(
        &mut self,
        mut skipped: usize,
        mut skip_remaining: usize,
    ) -> crate::Result<usize> {
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

impl<T> Gallery for PagedGallery<T>
where
    T: Pager,
{
    type Item = <<T as Pager>::Item as Downloadable>::Output;

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
        let skipped = 0;
        let skip_remaining = n;
        self.default_advance_by(skipped, skip_remaining)
    }
}

mod prelude {
    pub use crate::{
        error::{Error, ExtractionFailure, UnsupportedError},
        gallery::{
            Downloadable, NamedGalleryItem, Page, PagedGallery, Pager, ResponseGalleryItem,
            UnpagedGallery,
        },
    };
    pub use reqwest::blocking::{Client, Response};
    pub use std::collections::VecDeque;

    pub static USER_AGENT: &str =
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/106.0.0.0 Safari/537.36";

    pub struct ImageLink(String);

    impl<T: Into<String>> From<T> for ImageLink {
        fn from(value: T) -> Self {
            ImageLink(value.into())
        }
    }

    impl Downloadable for ImageLink {
        type Context = Client;

        type Output = ResponseGalleryItem;

        fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
            Ok(ResponseGalleryItem::new(context.get(self.0).send()?))
        }
    }
}
