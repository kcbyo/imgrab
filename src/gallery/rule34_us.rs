use regex::Regex;

use super::prelude::*;

// Rule34.us seems to have the id for each post in the link. The following regular expression
// seems to work:
//
//      <a id="(\d+)"
//

pub fn extract(url: &str) -> crate::Result<(PagedGallery<Rule34Pager>, Option<String>)> {
    let query = get_query(url)?;
    let pager = Rule34Pager {
        query,
        page: 0,
        is_complete: false,
    };

    let gallery = PagedGallery {
        context: super::build_client(),
        pager,
        current: Page::Empty,
    };

    match get_single_tag(&gallery.pager.query).map(|x| x.to_owned()) {
        Some(tag) => Ok((gallery, Some(tag))),
        None => Ok((gallery, None)),
    }
}

pub struct Rule34Pager {
    query: String,
    page: usize,
    is_complete: bool,
}

impl Pager for Rule34Pager {
    type Context = Client;

    type Item = GalleryItemUrl;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        todo!()
    }
}

pub struct GalleryItemUrl(String);

impl Downloadable for GalleryItemUrl {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        todo!()
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
