use regex::Regex;

use super::{prelude::*, Gallery};

pub fn extract(url: &str) -> crate::Result<(PagedGallery<Rule34Pager>, Option<String>)> {
    let query = get_query(url)?;
    let pager = Rule34Pager::new(query);
    let gallery = PagedGallery {
        context: Context::new(),
        pager,
        current: Page::Empty,
    };

    match get_single_tag(&gallery.pager.query).map(|x| x.to_owned()) {
        Some(tag) => Ok((gallery, Some(tag))),
        None => Ok((gallery, None)),
    }
}

pub struct Context {
    client: Client,
    image_url_expr: Regex,
}

impl Context {
    fn new() -> Self {
        Self {
            client: super::build_client(),
            image_url_expr: Regex::new(
                r#"a href="([^"]+/images/[^"]+)"|img src="([^"]+/images/[^"]+)""#,
            )
            .unwrap(),
        }
    }
}

pub struct Rule34Gallery {
    inner: PagedGallery<Rule34Pager>,
}

impl Gallery for Rule34Gallery {
    type Item = ResponseGalleryItem;

    fn next(&mut self) -> Option<crate::Result<Self::Item>> {
        self.inner.next()
    }

    fn advance_by(&mut self, n: usize) -> crate::Result<usize> {
        const PAGE_SIZE: usize = 42;

        let mut skipped = 0;
        let mut skip_remaining = 0;

        let advance_pages = n / PAGE_SIZE;
        if advance_pages > 0 {
            skipped = advance_pages * PAGE_SIZE;
            skip_remaining -= skipped;
            self.inner.pager.page += skipped;
            self.inner.current = self.inner.pager.next_page(&self.inner.context)?;
        }

        self.inner.default_advance_by(skipped, skip_remaining)
    }
}

pub struct Rule34Pager {
    query: String,
    page: usize,
    is_complete: bool,

    gallery_item_id_expr: Regex,
}

impl Rule34Pager {
    fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            page: 0,
            is_complete: false,
            gallery_item_id_expr: Regex::new(r#"<a id="(\d+)""#).unwrap(),
        }
    }

    fn format_request(&self) -> String {
        // https://rule34.us/index.php?r=posts/index&q=korra+sweat+
        // https://rule34.us/index.php?r=posts/index&q=korra+sweat+&page=1
        let query = &self.query;
        let page = &self.page;
        format!("https://rule34.us/index.php?r=posts/index&q={query}&page={page}")
    }
}

impl Pager for Rule34Pager {
    type Context = Context;

    type Item = GalleryItemId;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        if self.is_complete {
            return Ok(Page::Empty);
        }

        let request = self.format_request();
        self.page += 1;
        let text = context.client.get(&request).send()?.text()?;

        // https://rule34.us/index.php?r=posts/view&id=4597827
        // https://rule34.us/index.php?r=posts/view&id=4597826
        // https://rule34.us/index.php?r=posts/view&id=4545892

        let identifiers = self
            .gallery_item_id_expr
            .captures_iter(&text)
            .filter_map(|cx| cx.get(1))
            .map(|cx| GalleryItemId(cx.as_str().into()));

        Ok(identifiers.collect())
    }
}

pub struct GalleryItemId(String);

impl GalleryItemId {
    fn page_url(&self) -> String {
        let id = &self.0;
        format!("https://rule34.us/index.php?r=posts/view&id={id}")
    }
}

impl Downloadable for GalleryItemId {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let text = context.client.get(&self.page_url()).send()?.text()?;
        let url = context
            .image_url_expr
            .captures_iter(&text)
            .filter_map(|cx| cx.get(1).or_else(|| cx.get(2)))
            .next()
            .ok_or_else(|| {
                Error::Extraction(
                    ExtractionFailure::ImageUrl,
                    String::from("unable to extract image metadata"),
                )
            })?
            .as_str();

        Ok(context
            .client
            .get(url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

fn get_query(url: &str) -> crate::Result<String> {
    let query_expr = Regex::new(r#"(\?|&)q=([^&]+)"#).unwrap();
    Ok(query_expr
        .captures(url)
        .and_then(|cx| cx.get(2))
        .ok_or_else(|| Error::Unsupported(UnsupportedError::Route, url.into()))?
        .as_str()
        .into())
}

fn get_single_tag(tags: &str) -> Option<&str> {
    let mut tags = tags.split('+');
    let single = tags.next();
    single.filter(|_| tags.next().is_none())
}

#[cfg(test)]
mod tests {
    #[test]
    fn get_query() {
        let url = "https://rule34.us/index.php?r=posts/index&q=korra+sweat+";
        let query = super::get_query(url).unwrap();
        assert_eq!("korra+sweat+", query);
    }

    #[test]
    fn get_single_tag_rejects_multiple_tags() {
        let tags = "hello+world";
        let single = super::get_single_tag(tags);
        assert!(single.is_none());
    }

    #[test]
    fn get_single_tag_selects_single_tags() {
        let tags = "hello";
        let single = super::get_single_tag(tags);
        assert_eq!(Some("hello"), single);
    }
}
