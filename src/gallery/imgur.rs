use serde::Deserialize;

use crate::{
    config::{Configuration, Key},
    gallery::prelude::*,
};

use super::{UnpagedGallery, UreqGalleryItem};

#[derive(Clone, Debug, Deserialize)]
struct ResponseModel<T> {
    data: T,
    success: bool,
    status: i16,
}

#[derive(Clone, Debug, Deserialize)]
struct GalleryModel {
    id: String,
    title: String,
    link: String,
    images: VecDeque<ImageModel>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ImageModel {
    id: String,
    width: u32,
    height: u32,
    size: u64,
    link: String,
    mp4: Option<String>,
}

impl Downloadable for ImageModel {
    type Context = Context;
    type Output = UreqGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let link = self.mp4.unwrap_or(self.link);
        Ok(context.get(&link).map(UreqGalleryItem::new)?)
    }
}

pub fn extract(url: &str) -> crate::Result<UnpagedGallery<ImageModel>> {
    let context = Context::try_with_config()?;

    if url.contains("imgur.com/a/") {
        let images = query_album(&context, url)?;
        return Ok(UnpagedGallery {
            context,
            items: images,
        });
    }

    if url.contains("imgur.com/gallery/") {
        let images = query_gallery(&context, url)?;
        return Ok(UnpagedGallery {
            context,
            items: images,
        });
    }

    let image = query_image(&context, url)?;
    let mut images = VecDeque::with_capacity(1);
    images.push_back(image);
    Ok(UnpagedGallery {
        context,
        items: images,
    })
}

pub struct Context {
    agent: Agent,
    auth_header_content: String,
}

impl Context {
    fn try_with_config() -> crate::Result<Self> {
        let config = Configuration::init();
        let imgur_client_id = config.get_config(Key::ImgurClientId)?;

        Ok(Self {
            agent: AgentBuilder::new().user_agent(USER_AGENT).build(),
            auth_header_content: format!("Client-ID {}", imgur_client_id),
        })
    }

    fn get(&self, url: &str) -> Result<ureq::Response, ureq::Error> {
        self.agent
            .get(url)
            .set("Accept", "text/json")
            .set("Authorization", &self.auth_header_content)
            .call()
    }
}

fn query_album(context: &Context, url: &str) -> crate::Result<VecDeque<ImageModel>> {
    let response: ResponseModel<VecDeque<ImageModel>> = context
        .get(&format!(
            "https://api.imgur.com/3/album/{}/images",
            last_segment(url)?
        ))?
        .into_json()?;
    Ok(response.data)
}

fn query_gallery(context: &Context, url: &str) -> crate::Result<VecDeque<ImageModel>> {
    let response: ResponseModel<GalleryModel> = context
        .get(&format!(
            "https://api.imgur.com/3/gallery/album/{}",
            last_segment(url)?
        ))?
        .into_json()?;
    Ok(response.data.images)
}

fn query_image(context: &Context, url: &str) -> crate::Result<ImageModel> {
    let response: ResponseModel<ImageModel> = context
        .get(&format!(
            "https://api.imgur.com/3/image/{}",
            last_segment(url)?
        ))?
        .into_json()?;
    Ok(response.data)
}

fn last_segment(address: &str) -> crate::Result<String> {
    let address = url::Url::parse(address)?;
    address
        .path_segments()
        .into_iter()
        .flatten()
        .last()
        .map(|x| x.to_string())
        .ok_or_else(|| {
            Error::Extraction(
                ExtractionFailure::Metadata,
                String::from("Failed to extract imgur image hash"),
            )
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn last_segment() {
        let actual = super::last_segment("https://imgur.com/a/gN55w").unwrap();
        let expected = "gN55w";
        assert_eq!(actual, expected);
    }
}
