use super::prelude::*;

use std::fmt::{self, Display};

use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, Deserialize)]
struct GalleryInfo {
    id: u64,
    media_id: String,
    images: Images,
    #[serde(rename = "num_pages")]
    pages: i32,
}

#[derive(Clone, Debug, Deserialize)]
struct Images {
    cover: ImageInfo,
    thumbnail: ImageInfo,
    pages: VecDeque<ImageInfo>,
}

#[derive(Clone, Debug, Deserialize)]
struct ImageInfo {
    #[serde(rename = "t")]
    format: ImageFormat,
}

#[derive(Copy, Clone, Debug)]
enum ImageFormat {
    Gif,
    Jpg,
    Png,
}

impl<'de> Deserialize<'de> for ImageFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct FormatVisitor;

        impl<'de> Visitor<'de> for FormatVisitor {
            type Value = ImageFormat;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string representing an image format in: gif, jpg, png")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                match value {
                    "g" => Ok(ImageFormat::Gif),
                    "j" => Ok(ImageFormat::Jpg),
                    "p" => Ok(ImageFormat::Png),

                    _ => Err(E::custom(format!("unknown format signifier: {}", value))),
                }
            }
        }

        deserializer.deserialize_str(FormatVisitor)
    }
}

impl Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageFormat::Gif => f.write_str("gif"),
            ImageFormat::Jpg => f.write_str("jpg"),
            ImageFormat::Png => f.write_str("png"),
        }
    }
}

pub fn extract(url: &str) -> crate::Result<NHentaiGallery> {
    let client = Client::builder().user_agent(super::USER_AGENT).build()?;

    // The gallery page serves no real purpose for us, because each of the image pages
    // includes a json packet which describes the book we're trying to download. Once we
    // have the gallery info, we'll store the media id and image formats for future use.

    let url = url.to_string() + "/1/";
    let document = client.get(&url).send()?.text()?;
    let pattern = Regex::new(r#"JSON\.parse\("(.+?)"\)"#).unwrap();
    let packet = pattern
        .captures(&document)
        .and_then(|x| x.get(1))
        .ok_or_else(|| {
            Error::Extraction(
                ExtractionFailure::Metadata,
                "Unable to get gallery metadata".into(),
            )
        })?
        .as_str()
        .replace("\\u0022", "\"");

    let gallery_info: GalleryInfo = serde_json::from_str(&packet).map_err(|e| {
        Error::Extraction(
            ExtractionFailure::Metadata,
            format!("Unable to parse gallery data: {}", e),
        )
    })?;

    Ok(NHentaiGallery {
        client,
        media_id: gallery_info.media_id,
        queue: gallery_info.images.pages,
        current_page: 0,
    })
}

pub struct NHentaiGallery {
    client: Client,
    media_id: String,
    queue: VecDeque<ImageInfo>,
    current_page: i32,
}

impl NHentaiGallery {
    fn next_image_url(&mut self) -> Option<String> {
        let format = self.queue.pop_front()?.format;
        self.current_page += 1;
        Some(format!(
            "https://i.nhentai.net/galleries/{}/{}.{}",
            self.media_id, self.current_page, format
        ))
    }
}

impl Gallery for NHentaiGallery {
    fn advance_by(&mut self, skip: usize) -> crate::Result<()> {
        if skip < self.queue.len() {
            self.queue.drain(..skip);
            self.current_page += skip as i32;
        } else {
            self.queue.clear();
        }
        Ok(())
    }

    fn next(&mut self) -> Option<crate::Result<GalleryItem>> {
        self.next_image_url().map(|url| {
            self.client
                .get(&url)
                .send()
                .map(|x| GalleryItem::new(url, x))
                .map_err(|e| e.into())
        })
    }
}
