use super::prelude::*;

use std::vec;

pub struct FitNakedGirls;

impl ReadGallery for FitNakedGirls {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
        let client = build_client()?;
        let content = client.get(url).send()?.text()?;
        Ok(Box::new(FngGallery {
            client,
            images: extract_images(&content).into_iter(),
        }))
    }
}

fn extract_images(content: &str) -> Vec<String> {
    use scraper::{Html, Selector};

    let item_selector = Selector::parse("div.album img,div.album source").unwrap();
    let document = Html::parse_document(content);
    document
        .select(&item_selector)
        .filter_map(|noderef| {
            let src = noderef.value().attr("src")?;
            match src.rfind('?') {
                Some(idx) => Some(src[..idx].into()),
                None => Some(src.into()),
            }
        })
        .collect()
}

pub struct FngGallery {
    client: Client,
    images: vec::IntoIter<String>,
}

impl Gallery for FngGallery {
    fn apply_skip(&mut self, skip: usize) -> crate::Result<()> {
        let buf: Vec<_> = self.images.by_ref().skip(skip).collect();
        self.images = buf.into_iter();
        Ok(())
    }
}

impl Iterator for FngGallery {
    type Item = crate::Result<GalleryItem>;

    fn next(&mut self) -> Option<Self::Item> {
        self.images.next().map(|url| {
            self.client
                .get(&url)
                .send()
                .map(|x| GalleryItem::new(url, x))
                .map_err(|e| e.into())
        })
    }
}

fn build_client() -> crate::Result<Client> {
    use reqwest::header::{self, HeaderValue};

    let builder = Client::builder();
    let mut headers = header::HeaderMap::new();

    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("imgrab 0.1.4+"),
    );

    Ok(builder.default_headers(headers).build()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn blearch() {
        let content = include_str!("../../resource/fitnakedgirls/gallery.html");
        let links = super::extract_images(content);
        assert_eq!(129, links.len());
    }
}
