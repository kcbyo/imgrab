// Impl for hdporn.pics
//
// As far as I can tell, galleries are not paged. This implementation
// ONLY covers galleries; the search function is untouched and confusing.

use crate::gallery::prelude::*;

pub fn extract(url: &str) -> crate::Result<(UnpagedGallery<Image>, Option<String>)> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .build()?;
    let text = client.get(url).send()?.text()?;
    let document = nipper::Document::from(&text);

    let links = document
        .select("div.sh-section__images a")
        .iter()
        .filter_map(|cx| cx.attr("href"))
        .map(|url| Image(url.into()));

    Ok((
        UnpagedGallery {
            context: client,
            items: links.collect(),
        },
        None,
    ))
}

pub struct Image(String);

impl Downloadable for Image {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(ResponseGalleryItem::new(context.get(self.0).send()?))
    }
}
