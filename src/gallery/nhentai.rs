/*!
The operation of this gallery is a little bit insane. The way it works is
as follows:

We store a list of *image formats* when we first query the gallery. The
format specifier becomes a part of a URL of te form

> https://i.nhentai.net/galleries/<media id>/<page count>.<format>

This list gives us a count for the total gallery length, but we can mostly
infer filenames save for the file type itself. In addition, most of the code
here seems to have been written to support deserialization rather than for
the gallery itself.
*/

use std::fmt::{self, Display};

use regex::Regex;
use serde::{Deserialize, Deserializer};

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<UnpagedGallery<ImageToken>> {
    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();

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

    // There's no real reason to use i32 over usize, but the documentation
    // suggested trying it out. Whatever.
    let tokens = (1i32..)
        .zip(gallery_info.images.pages.into_iter())
        .map(|(idx, info)| ImageToken {
            idx,
            fmt: info.format,
        });

    Ok(UnpagedGallery {
        context: Context {
            client,
            media_id: gallery_info.media_id,
        },
        items: tokens.collect(),
    })
}

pub struct ImageToken {
    idx: i32,
    fmt: ImageFormat,
}

impl Downloadable for ImageToken {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let ImageToken { idx, fmt } = self;
        let url = format!(
            "https://i.nhentai.net/galleries/{}/{}.{}",
            context.media_id, idx, fmt
        );

        Ok(context
            .client
            .get(&url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

pub struct Context {
    client: Client,
    media_id: String,
}

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
    pub format: ImageFormat,
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
