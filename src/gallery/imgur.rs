use serde::Deserialize;

use crate::{
    config::{Configuration, Key},
    gallery::prelude::*,
};

use super::{ResponseGalleryItem, UnpagedGallery};

#[derive(Clone, Debug, Deserialize)]
struct ResponseModel<T> {
    data: T,
    // success: bool,
    // status: i16,
}

#[derive(Clone, Debug, Deserialize)]
struct GalleryModel {
    // id: String,
    // title: String,
    // link: String,
    images: VecDeque<ImageModel>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ImageModel {
    // id: String,
    // width: u32,
    // height: u32,
    // size: u64,
    link: String,
    mp4: Option<String>,
}

impl Downloadable for ImageModel {
    type Context = Context;
    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let link = self.mp4.unwrap_or(self.link);
        Ok(context
            .client
            .get(&link)
            .send()
            .map(ResponseGalleryItem::new)?)
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
    client: Client,
}

impl Context {
    fn try_with_config() -> crate::Result<Self> {
        use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};

        let config = Configuration::init();
        let imgur_client_id = config.get_config(Key::ImgurClientId)?;

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("text/json"));
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Client-ID {}", imgur_client_id)).unwrap(),
        );

        Ok(Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .default_headers(headers)
                .build()
                .unwrap(),
        })
    }
}

fn query_album(context: &Context, url: &str) -> crate::Result<VecDeque<ImageModel>> {
    let response: ResponseModel<VecDeque<ImageModel>> = context
        .client
        .get(&format!(
            "https://api.imgur.com/3/album/{}/images",
            last_segment(url)?
        ))
        .send()?
        .json()?;
    Ok(response.data)
}

fn query_gallery(context: &Context, url: &str) -> crate::Result<VecDeque<ImageModel>> {
    let response: ResponseModel<GalleryModel> = context
        .client
        .get(&format!(
            "https://api.imgur.com/3/gallery/album/{}",
            last_segment(url)?
        ))
        .send()?
        .json()?;
    Ok(response.data.images)
}

fn query_image(context: &Context, url: &str) -> crate::Result<ImageModel> {
    let response: ResponseModel<ImageModel> = context
        .client
        .get(&format!(
            "https://api.imgur.com/3/image/{}",
            last_segment(url)?
        ))
        .send()?
        .json()?;
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
