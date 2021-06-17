use std::{
    borrow::Cow,
    collections::VecDeque,
    io::{self, Write},
};

// pub mod ehentai;
// pub mod fitnakedgirls;
// pub mod gelbooru;
// pub mod girlswithmuscle;
// pub mod hentai_foundry;
// pub mod nhentai;
// pub mod nsfwalbum;
// pub mod rule34;
// pub mod sankakubeta;
pub mod flist;
pub mod imgur;
pub mod thefitgirlz;

use crate::storage::NameContext;

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
    fn write<W: Write + ?Sized>(self, writer: &mut W) -> io::Result<u64>;
}

/// A basic gallery item implemented on only a bare [`ureq::Response`]
pub struct UreqGalleryItem {
    response: ureq::Response,
}

impl UreqGalleryItem {
    pub fn new(response: ureq::Response) -> Self {
        Self { response }
    }
}

impl GalleryItem for UreqGalleryItem {
    fn context(&self) -> NameContext {
        NameContext::from_response(&self.response)
    }

    fn write<W: io::Write + ?Sized>(self, writer: &mut W) -> io::Result<u64> {
        io::copy(&mut self.response.into_reader(), writer)
    }
}

/// A gallery item with an explicitly-overridden name
pub struct NamedGalleryItem {
    name: String,
    response: ureq::Response,
}

impl NamedGalleryItem {
    pub fn new(response: ureq::Response, name: impl Into<String>) -> Self {
        Self {
            response,
            name: name.into(),
        }
    }
}

impl GalleryItem for NamedGalleryItem {
    fn context(&self) -> NameContext {
        NameContext::new(self.response.get_url(), Some(Cow::from(&self.name)))
    }

    fn write<W: io::Write + ?Sized>(self, writer: &mut W) -> io::Result<u64> {
        io::copy(&mut self.response.into_reader(), writer)
    }
}

pub trait Downloadable {
    type Context;
    type Output: GalleryItem;

    /// Materialize a downloadable item as a gallery item.
    fn download(self, agent: &Self::Context) -> crate::Result<Self::Output>;
}

pub trait Pager {
    type Context;
    type Item: Downloadable<Context = Self::Context>;
    fn next_page(&mut self, agent: &Self::Context) -> Option<crate::Result<VecDeque<Self::Item>>>;
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
    current_page: VecDeque<T::Item>,
}

impl<T> Gallery for PagedGallery<T>
where
    T: Pager,
{
    type Item = <<T as Pager>::Item as Downloadable>::Output;

    fn next(&mut self) -> Option<crate::Result<Self::Item>> {
        if self.current_page.is_empty() {
            match self.pager.next_page(&self.context)? {
                Ok(next_page) => self.current_page = next_page,
                Err(e) => return Some(Err(e)),
            }

            return self.next();
        }

        let item = self.current_page.pop_front()?;
        Some(item.download(&self.context))
    }

    fn advance_by(&mut self, n: usize) -> crate::Result<usize> {
        let mut skipped = 0;
        let mut skip_remaining = n;

        loop {
            if self.current_page.is_empty() {
                self.current_page = match self.pager.next_page(&self.context) {
                    Some(Ok(next_page)) => next_page,
                    Some(Err(e)) => return Err(e),
                    None => return Ok(0),
                };
            }

            if self.current_page.len() > skip_remaining {
                let _ = self.current_page.drain(..skip_remaining);
                return Ok(skipped + skip_remaining);
            } else {
                skipped += self.current_page.len();
                skip_remaining -= self.current_page.len();
                self.current_page.clear();
            }
        }
    }
}

mod prelude {
    pub use crate::{
        error::{Error, ExtractionFailure, UnsupportedError},
        gallery::{
            Downloadable, NamedGalleryItem, PagedGallery, Pager, UnpagedGallery, UreqGalleryItem,
        },
    };
    pub use std::collections::VecDeque;
    pub use ureq::{Agent, AgentBuilder};

    pub static USER_AGENT: &str =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:89.0) Gecko/20100101 Firefox/89.0";
}
