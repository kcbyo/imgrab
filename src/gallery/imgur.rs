use std::vec;

use serde::Deserialize;

use crate::gallery::prelude::*;

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
    images: Vec<ImageModel>,
}

#[derive(Clone, Debug, Deserialize)]
struct ImageModel {
    id: String,
    width: u32,
    height: u32,
    size: u64,
    link: String,
}

pub struct ImgurAlbum;

impl ReadGallery for ImgurAlbum {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
        let client = build_client()?;
        let images = query_album(&client, url)?.into_iter();
        Ok(Box::new(ImgurMultipleGallery { client, images }))
    }
}

pub struct ImgurGallery;

impl ReadGallery for ImgurGallery {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
        let client = build_client()?;
        let images = query_gallery(&client, url)?.into_iter();
        Ok(Box::new(ImgurMultipleGallery { client, images }))
    }
}

pub struct ImgurSingle;

impl ReadGallery for ImgurSingle {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
        let client = build_client()?;
        let image = query_image(&client, url)?;
        Ok(Box::new(ImgurSingleGallery {
            client,
            image: Some(image),
        }))
    }
}

pub struct ImgurMultipleGallery {
    client: Client,
    images: vec::IntoIter<ImageModel>,
}

impl Gallery for ImgurMultipleGallery {
    fn apply_skip(&mut self, skip: usize) -> crate::Result<()> {
        let images: Vec<_> = self.images.by_ref().skip(skip).collect();
        self.images = images.into_iter();
        Ok(())
    }
}

impl Iterator for ImgurMultipleGallery {
    type Item = crate::Result<GalleryItem>;

    fn next(&mut self) -> Option<Self::Item> {
        let image = self.images.next()?;
        match self.client.get(&image.link).send() {
            Ok(response) => Some(Ok(GalleryItem::new(image.link, response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}

pub struct ImgurSingleGallery {
    client: Client,
    image: Option<ImageModel>,
}

impl Gallery for ImgurSingleGallery {
    fn apply_skip(&mut self, skip: usize) -> crate::Result<()> {
        if skip > 0 {
            self.image = None;
        }
        Ok(())
    }
}

impl Iterator for ImgurSingleGallery {
    type Item = crate::Result<GalleryItem>;

    fn next(&mut self) -> Option<Self::Item> {
        let image = self.image.take()?;
        match self.client.get(&image.link).send() {
            Ok(response) => Some(Ok(GalleryItem::new(image.link, response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}

fn query_album(client: &Client, url: &str) -> crate::Result<Vec<ImageModel>> {
    let response: ResponseModel<Vec<ImageModel>> = client
        .get(&format!(
            "https://api.imgur.com/3/album/{}/images",
            last_segment(url)?
        ))
        .send()?
        .json()?;
    Ok(response.data)
}

fn query_gallery(client: &Client, url: &str) -> crate::Result<Vec<ImageModel>> {
    let response: ResponseModel<GalleryModel> = client
        .get(&format!(
            "https://api.imgur.com/3/gallery/album/{}",
            last_segment(url)?
        ))
        .send()?
        .json()?;
    Ok(response.data.images)
}

fn query_image(client: &Client, url: &str) -> crate::Result<ImageModel> {
    let response: ResponseModel<ImageModel> = client
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

fn build_client() -> crate::Result<Client> {
    use crate::config::{Configuration, Key};
    use reqwest::header::{self, HeaderValue};

    let config = Configuration::init();
    let imgur_client_id = config.get_config(Key::ImgurClientId)?;
    let builder = Client::builder();

    let mut headers = header::HeaderMap::new();

    headers.insert(header::ACCEPT, HeaderValue::from_static("text/json"));
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("imgrab 0.1.4+"),
    );
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Client-ID {}", imgur_client_id)).unwrap(),
    );

    Ok(builder.default_headers(headers).build()?)
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
