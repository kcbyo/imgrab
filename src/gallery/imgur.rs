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

pub fn extract(url: &str) -> crate::Result<ImgurGallery> {
    let client = build_client()?;

    if url.contains("imgur.com/a/") {
        let images = query_album(&client, url)?;
        return Ok(ImgurGallery {
            client,
            idx: 0,
            images,
        });
    }

    if url.contains("imgur.com/gallery/") {
        let images = query_gallery(&client, url)?;
        return Ok(ImgurGallery {
            client,
            idx: 0,
            images,
        });
    }

    let image = query_image(&client, url)?;
    Ok(ImgurGallery {
        client,
        idx: 0,
        images: vec![image],
    })
}

pub struct ImgurGallery {
    client: Client,
    idx: usize,
    images: Vec<ImageModel>,
}

impl Gallery for ImgurGallery {
    fn next(&mut self) -> Option<crate::Result<GalleryItem>> {
        match self.idx {
            idx if idx < self.images.len() => {
                self.idx += 1;
                let link = self.images[idx].link.clone();
                match self.client.get(&link).send() {
                    Ok(response) => Some(Ok(GalleryItem::new(link, response))),
                    Err(e) => Some(Err(e.into())),
                }
            }

            _ => None,
        }
    }

    fn advance_by(&mut self, n: usize) -> crate::Result<()> {
        self.idx += n;
        Ok(())
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
