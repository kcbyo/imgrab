use std::ops::RangeInclusive;

use regex::Regex;

use super::{build_client, prelude::*};

pub fn extract(url: &str) -> crate::Result<(PagedGallery<FappeningBookPager>, Option<String>)> {
    let client = build_client();
    let gallery_name = name_from_url(url);

    let text = client.get(url).send()?.text()?;
    let pages = get_pages(&text)?;

    Ok((
        PagedGallery {
            current: Page::Empty,
            context: Context::new(client),
            pager: FappeningBookPager::new(url, pages),
        },
        gallery_name.map(|name| name.into()),
    ))
}

pub struct FappeningBookPager {
    base_url: String,
    pages: RangeInclusive<usize>,
}

impl FappeningBookPager {
    fn new(url: &str, pages: RangeInclusive<usize>) -> Self {
        Self {
            base_url: url.into(),
            pages,
        }
    }
}

impl FappeningBookPager {
    fn build_url(&self, idx: usize) -> String {
        // https://fappeningbook.com/<model-name-here>/2/
        // Note that the base url must end with a / and probably already does
        format!("{}{}/", self.base_url, idx)
    }
}

impl Pager for FappeningBookPager {
    type Context = Context;

    type Item = Image;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        let current = match self.pages.next() {
            Some(idx) => idx,
            None => return Ok(Page::Empty),
        };

        match current {
            1 => context.fetch_page(&self.base_url),
            n => context.fetch_page(&self.build_url(n)),
        }
    }
}

pub struct Context {
    client: Client,
    img_src_re: Regex,
}

impl Context {
    fn new(client: Client) -> Self {
        Self {
            client,
            img_src_re: Regex::new(r#"data-orig="(.+?)""#).unwrap(),
        }
    }

    fn fetch_page(&self, url: &str) -> crate::Result<Page<Image>> {
        let text = self.client.get(url).send()?.text()?;
        let links: VecDeque<_> = self
            .img_src_re
            .captures_iter(&text)
            .filter_map(|cx| cx.get(1).map(|cx| Image(cx.as_str().to_owned())))
            .collect();

        if !links.is_empty() {
            Ok(Page::Items(links))
        } else {
            Ok(Page::Empty)
        }
    }
}

pub struct Image(String);

impl Downloadable for Image {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(context
            .client
            .get(self.0)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

fn name_from_url(url: &str) -> Option<&str> {
    let re = Regex::new(r#"fappeningbook.com/(.+?)/"#).unwrap();
    re.captures(url)
        .and_then(|cx| cx.get(1).map(|cx| cx.as_str()))
}

fn get_pages(text: &str) -> crate::Result<RangeInclusive<usize>> {
    let re = Regex::new(r#"Page (\d+) of (\d+)"#).unwrap();
    if let Some((n, m)) = re
        .captures(text)
        .and_then(|cx| cx.get(1).and_then(|n| cx.get(2).map(|m| (n, m))))
    {
        let n: usize = n.as_str().parse().map_err(|_| {
            Error::Extraction(
                ExtractionFailure::Metadata,
                String::from("unable to get gallery count"),
            )
        })?;
        let m = m.as_str().parse().map_err(|_| {
            Error::Extraction(
                ExtractionFailure::Metadata,
                String::from("unable to get gallery count"),
            )
        })?;
        Ok(n..=m)
    } else {
        Err(Error::Extraction(
            ExtractionFailure::Metadata,
            String::from("unable to get gallery count"),
        ))
    }
}
