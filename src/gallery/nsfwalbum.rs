use regex::Regex;

use super::prelude::*;

// FIXME: This almost works, but it's actually downloading thumbnails instead of full-size images.

pub fn extract(url: &str) -> crate::Result<(UnpagedGallery<NsfwImageId>, Option<String>)> {
    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();
    let pattern = Regex::new(r#"data-img-id="(\d+)""#).unwrap();
    let content = client.get(url).send()?.text()?;
    let images = pattern
        .captures_iter(&content)
        .filter_map(|x| x.get(1).map(|x| x.as_str().to_owned()))
        .map(NsfwImageId);

    Ok((
        UnpagedGallery {
            context: Context::with_client(client),
            items: images.collect(),
        },
        None,
    ))
}

pub struct Context {
    client: Client,
    pattern: Regex,
}

impl Context {
    fn with_client(client: Client) -> Self {
        Self {
            client,
            pattern: Regex::new(r#"giraffe\.annihilate\("([^"]+)", (\d+)\)"#).unwrap(),
        }
    }

    fn request_image(&self, id: &str) -> crate::Result<Response> {
        let url = format_stage_one_url(id);
        let image_content = self.client.get(&url).send()?.text()?;
        let (giraffe, salt) = self.extract_params(&image_content)?;
        let url = format_stage_two_url(id, giraffe, salt);
        Ok(self.client.get(&url).send()?)
    }

    fn extract_params<'a>(&self, image_content: &'a str) -> crate::Result<(&'a str, i32)> {
        let captures = self.pattern.captures(image_content).ok_or_else(|| {
            Error::Extraction(
                ExtractionFailure::ImageUrl,
                String::from("Unable to extract download parameters"),
            )
        })?;

        Ok((
            captures.get(1).unwrap().as_str(),
            captures
                .get(2)
                .unwrap()
                .as_str()
                .parse::<i32>()
                .map_err(|e| {
                    Error::Other(String::from("Unable to parse giraffe salt"), Box::new(e))
                })?,
        ))
    }
}

pub struct NsfwImageId(String);

impl Downloadable for NsfwImageId {
    type Context = Context;

    type Output = NamedGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let id = self.0;
        context
            .request_image(&id)
            .map(|response| NamedGalleryItem::new(response, id + ".jpg"))
    }
}

fn format_stage_one_url(id: &str) -> String {
    static URL_BASE: &str = "https://nsfwalbum.com/photo/";
    URL_BASE.to_owned() + id
}

fn format_stage_two_url(id: &str, giraffe: &str, salt: i32) -> String {
    static URL_BASE: &str = "https://nsfwalbum.com/imageProxy.php?photoId=";
    static URL_SEPARATOR: &str = "&spirit=";

    let a = annihilate(giraffe, salt);
    URL_BASE.to_owned() + id + URL_SEPARATOR + &urlencoding::encode(&a)
}

// Do not ask. I have no fucking idea.
fn annihilate(giraffe: &str, salt: i32) -> String {
    giraffe
        .bytes()
        .map(|u| {
            let u = u as i32;
            let e = u ^ salt;

            if (0..256).contains(&e) {
                e as u8 as char
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn can_annihilate() {
        let actual = super::annihilate("83|93|93|93|93|a7|a7|96|", 6);
        let expected = ">5z?5z?5z?5z?5zg1zg1z?0z";
        assert_eq!(actual, expected);
    }
}
