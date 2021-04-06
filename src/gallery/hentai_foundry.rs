use super::prelude::*;

use std::ops::RangeInclusive;

use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct SetFiltersRequest {
    // I'm hoping this isn't actually required, but we'll see.
    #[serde(rename = "YII_CSRF_TOKEN")]
    token: String,

    // All this garbage is... Whatever.
    rating_nudity: u8,
    rating_violence: u8,
    rating_profanity: u8,
    rating_racism: u8,
    rating_sex: u8,
    rating_spoilers: u8,
    rating_yaoi: u8,
    rating_yuri: u8,
    rating_teen: u8,
    rating_guro: u8,
    rating_furry: u8,
    rating_beast: u8,
    rating_male: u8,
    rating_female: u8,
    rating_futa: u8,
    rating_other: u8,
    rating_scat: u8,
    rating_incest: u8,
    rating_rape: u8,
    filter_media: &'static str,
    filter_order: &'static str,
    filter_type: u8,
}

impl SetFiltersRequest {
    fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            rating_nudity: 3,
            rating_violence: 3,
            rating_profanity: 3,
            rating_racism: 3,
            rating_sex: 3,
            rating_spoilers: 3,
            rating_yaoi: 1,
            rating_yuri: 1,
            rating_teen: 1,
            rating_guro: 1,
            rating_furry: 1,
            rating_beast: 1,
            rating_male: 1,
            rating_female: 1,
            rating_futa: 1,
            rating_other: 1,
            rating_scat: 1,
            rating_incest: 1,
            rating_rape: 1,
            filter_media: "A",
            filter_order: "date_new",
            filter_type: 0,
        }
    }
}

pub struct HentaiFoundry;

impl ReadGallery for HentaiFoundry {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
        let mut segments = url.split('/');
        let _ = segments.find(|&x| x == "user");

        if let Some(user) = segments.next() {
            user_gallery(&format!(
                "https://www.hentai-foundry.com/pictures/user/{}",
                user
            ))
        } else {
            Err(Error::Unsupported(UnsupportedError::Route, url.into()))
        }
    }
}

fn user_gallery(url: &str) -> crate::Result<DynamicGallery> {
    // First, build the client and get initial content using the enterAgree=1 param.
    // This content contains nothing good. We're going to use it to grab a CSRF token
    // and submit a filter update. Kind of silly we need to do this every time.
    let client = build_client()?;
    let base_url = truncate_url(url)?;
    let agree = base_url.to_string() + "?enterAgree=1";
    let content = client.get(&agree).send()?.text()?;

    // Extract the csrf token from the initial content and update filter settings.
    // I'm a little worried about this sending so many requests so quickly. Might be
    // a good idea to throw in a wait here somewhere, but testing shows that this
    // DOES work, so....
    let token = read_csrf_token(&content)?;
    update_filters(&client, token)?;
    let content = client.get(url).send()?.text()?;

    let pages = 2..=read_pages(&content).ok_or_else(|| {
        Error::Extraction(
            ExtractionFailure::Metadata,
            String::from("Unable to extract page count"),
        )
    })?;

    let image_pattern = Regex::new(r#"src="//pictures.hentai-foundry.com/(.+?)""#).unwrap();
    let full_image_pattern = Regex::new(r#"this.src=&#039;(.+)&#039;;"#).unwrap();
    let image_selector = Selector::parse("div.galleryViewTable a.thumbLink").unwrap();
    let queue = read_links(&image_selector, &content);

    Ok(Box::new(HentaiFoundryUserGallery {
        client,
        base_url: base_url.into(),
        pages,
        queue,
        skip: None,
        image_pattern,
        full_image_pattern,
        image_selector,
    }))
}

pub struct HentaiFoundryUserGallery {
    client: Client,
    base_url: String,
    pages: RangeInclusive<usize>,
    queue: VecDeque<String>,
    skip: Option<usize>,

    image_pattern: Regex,
    full_image_pattern: Regex,
    image_selector: Selector,
}

impl HentaiFoundryUserGallery {
    fn retrieve_batch(&mut self, page: usize) -> crate::Result<usize> {
        let url = format!("{}/page/{}", self.base_url, page);
        let document = self.client.get(&url).send()?.text()?;
        self.queue = read_links(&self.image_selector, &document);
        Ok(self.queue.len())
    }

    fn next_image_link(&mut self, url: &str) -> crate::Result<String> {
        let page = String::from("https://www.hentai-foundry.com") + url;
        let text = self.client.get(&page).send()?.text()?;

        fn extract_by_pattern<'a>(pattern: &Regex, text: &'a str) -> Option<&'a str> {
            Some(pattern.captures(text).map(|x| x.get(1).unwrap())?.as_str())
        }

        // Images may be linked directly or thumbnailed.
        let link = extract_by_pattern(&self.image_pattern, &text)
            .or_else(|| extract_by_pattern(&self.full_image_pattern, &text))
            .ok_or_else(|| Error::Extraction(ExtractionFailure::ImageUrl, url.into()))?;

        Ok(String::from("https://pictures.hentai-foundry.com/") + link)
    }
}

impl Gallery for HentaiFoundryUserGallery {
    fn apply_skip(&mut self, skip: usize) -> crate::Result<()> {
        self.skip = Some(skip);
        Ok(())
    }
}

impl Iterator for HentaiFoundryUserGallery {
    type Item = crate::Result<GalleryItem>;

    fn next(&mut self) -> Option<Self::Item> {
        // These two if blocks represent a substantial amount of recursion, but
        // who the hell has a gallery that big?
        if self.queue.is_empty() {
            match self.pages.next() {
                Some(page) => {
                    if let Err(e) = self.retrieve_batch(page) {
                        return Some(Err(e));
                    }
                    return self.next();
                }
                None => return None,
            }
        }

        if let Some(skip) = self.skip.take() {
            if skip > self.queue.len() {
                self.skip = Some(skip - self.queue.len());
                self.queue.clear();
                return self.next();
            } else {
                self.queue.drain(..skip);
            }
        }

        let next_link = self.queue.pop_front()?;
        let image_link = match self.next_image_link(&next_link) {
            Ok(link) => link,
            Err(e) => return Some(Err(e)),
        };

        Some(
            self.client
                .get(&image_link)
                .send()
                .map(|x| GalleryItem::new(image_link, x))
                .map_err(|e| e.into()),
        )
    }
}

fn truncate_url(url: &str) -> crate::Result<&str> {
    let pattern = Regex::new(r#".+pictures/user/[^/?]+"#).unwrap();
    pattern.find(url).map(|x| x.as_str()).ok_or_else(|| {
        Error::Extraction(
            ExtractionFailure::Metadata,
            String::from("Unable to extract base url"),
        )
    })
}

fn read_csrf_token(content: &str) -> crate::Result<&str> {
    let pattern = Regex::new(r#"type="hidden" value="([^"]+)" name="YII_CSRF_TOKEN""#).unwrap();
    pattern
        .captures(content)
        .and_then(|x| x.get(1).map(|x| x.as_str()))
        .ok_or_else(|| {
            Error::Extraction(
                ExtractionFailure::Metadata,
                String::from("Unable to extract csrf token"),
            )
        })
}

fn update_filters(client: &Client, token: &str) -> crate::Result<()> {
    let filters = SetFiltersRequest::new(token);
    client
        .post("https://www.hentai-foundry.com/site/filters")
        .form(&filters)
        .send()?;
    Ok(())
}

fn read_pages(content: &str) -> Option<usize> {
    let pattern = Regex::new(r#"class="last"><a href="/pictures/user/.+/page/(\d+)"#).unwrap();
    pattern.captures(content)?.get(1)?.as_str().parse().ok()
}

fn read_links<'a>(selector: &'a Selector, document: &'a str) -> VecDeque<String> {
    let document = Html::parse_document(document);
    document
        .select(selector)
        .filter_map(|x| x.value().attr("href"))
        .map(ToOwned::to_owned)
        .collect()
}

fn build_client() -> crate::Result<Client> {
    Ok(Client::builder()
        .user_agent(super::USER_AGENT)
        .cookie_store(true)
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::HentaiFoundry;
    use crate::gallery::ReadGallery;

    #[test]
    fn can_read_profile_links() {
        assert!(HentaiFoundry
            .read("https://www.hentai-foundry.com/user/AndavaNSFW")
            .is_ok());
        assert!(HentaiFoundry
            .read("https://www.hentai-foundry.com/user/AndavaNSFW/profile")
            .is_ok());
    }

    #[test]
    fn can_read_profile_gallery_links() {
        let result = HentaiFoundry.read("https://www.hentai-foundry.com/pictures/user/AndavaNSFW");
        assert!(result.is_ok());
    }
}
