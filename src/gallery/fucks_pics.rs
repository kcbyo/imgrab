use regex::Regex;
use scraper::{Html, Selector};

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<(UnpagedGallery<Image>, Option<String>)> {
    let client = super::build_client();
    let text = client.get(url).send()?.text()?;
    let gallery_base_url = get_gallery_base_url(url).ok_or_else(|| {
        Error::Extraction(
            ExtractionFailure::Metadata,
            "unable to determine gallery url".into(),
        )
    })?;

    let url_expr = Regex::new(r#"background:url\('([^']+)'\)"#).unwrap();
    let document = Html::parse_fragment(&text);
    let selector = Selector::parse("a").unwrap();
    let image_urls = document
        .select(&selector)
        .filter_map(|e| {
            e.value()
                .attr("href")?
                .starts_with(gallery_base_url)
                .then(|| e.inner_html())
        })
        .filter_map(|s| {
            url_expr
                .captures(&s)?
                .get(1)
                .map(|cx| cx.as_str().to_string())
        });

    Ok((
        UnpagedGallery {
            context: client,
            items: image_urls.map(Image).collect(),
        },
        get_gallery_name(&document),
    ))
}

fn get_gallery_base_url(url: &str) -> Option<&str> {
    let expr = Regex::new(r#"fucks.pics(/[^/]+)"#).unwrap();
    let cx = expr.captures(url)?;
    cx.get(1).map(|cx| cx.as_str())
}

fn get_gallery_name(document: &Html) -> Option<String> {
    let selector = Selector::parse("h3").unwrap();
    document
        .select(&selector)
        .next()
        .map(|element| element.inner_html())
}

pub struct Image(String);

impl Downloadable for Image {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(ResponseGalleryItem::new(context.get(self.0).send()?))
    }
}
