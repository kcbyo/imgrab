use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<(UnpagedGallery<FngUrl>, Option<String>)> {
    use scraper::{Html, Selector};

    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();
    let content = client.get(url).send()?.text()?;

    let item_selector = Selector::parse("div.album img,div.album source").unwrap();
    let document = Html::parse_document(&content);

    let gallery = UnpagedGallery {
        context: client,
        items: document
            .select(&item_selector)
            .filter_map(|noderef| {
                let src = noderef.value().attr("src")?;
                match src.rfind('?') {
                    Some(idx) => Some(src[..idx].into()),
                    None => Some(src.into()),
                }
            })
            .map(FngUrl)
            .collect(),
    };

    Ok((gallery, None))
}

pub struct FngUrl(String);

impl Downloadable for FngUrl {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(context.get(&self.0).send().map(ResponseGalleryItem::new)?)
    }
}
