use std::ops::RangeInclusive;

use regex::Regex;
use scraper::{Html, Selector};

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<(PagedGallery<HfPager>, Option<String>)> {
    let mut segments = url.split('/');
    let _ = segments.find(|&x| x == "user");

    if let Some(user) = segments.next() {
        let gallery_name = user.to_owned();
        user_gallery(&format!(
            "https://www.hentai-foundry.com/pictures/user/{}",
            user
        ))
        .map(|gallery| (gallery, Some(gallery_name)))
    } else {
        Err(Error::Unsupported(UnsupportedError::Route, url.into()))
    }
}

fn user_gallery(url: &str) -> crate::Result<PagedGallery<HfPager>> {
    // First, build the client and get initial content using the enterAgree=1 param.
    // This content contains nothing good. We're going to use it to grab a CSRF token
    // and submit a filter update. Kind of silly we need to do this every time.
    let client = build_client();
    let base_url = truncate_url(url)?;
    let agree = base_url.to_string() + "?enterAgree=1";
    let content = client.get(agree).send()?.text()?;

    // Extract the csrf token from the initial content and update filter settings.
    // I'm a little worried about this sending so many requests so quickly. Might be
    // a good idea to throw in a wait here somewhere, but testing shows that this
    // DOES work, so....
    let token = read_csrf_token(&content)?;
    update_filters(&client, token)?;
    let content = client.get(url).send()?.text()?;

    // For this gallery, we just go on and read the number of pages ahead of time.
    let pages = 2..=read_pages(&content).ok_or_else(|| {
        Error::Extraction(
            ExtractionFailure::Metadata,
            String::from("Unable to extract page count"),
        )
    })?;

    let context = Context::with_client(client);
    Ok(PagedGallery {
        current: Page::Items(context.read_links(&content)),
        pager: HfPager {
            base_url: base_url.into(),
            pages,
        },
        context,
    })
}

pub struct Context {
    client: Client,
    image_pattern: Regex,
    full_image_pattern: Regex,
    image_selector: Selector,
}

impl Context {
    fn with_client(client: Client) -> Self {
        Self {
            client,
            image_pattern: Regex::new(r#"src="//pictures.hentai-foundry.com/(.+?)""#).unwrap(),
            full_image_pattern: Regex::new(r#"this.src=&#039;(.+)&#039;;"#).unwrap(),
            image_selector: Selector::parse("div.galleryViewTable a.thumbLink").unwrap(),
        }
    }

    fn read_links(&self, document: &str) -> VecDeque<HfUrl> {
        let document = Html::parse_document(document);
        document
            .select(&self.image_selector)
            .filter_map(|x| x.value().attr("href"))
            .map(|url| HfUrl(url.into()))
            .collect()
    }
}

pub struct HfPager {
    base_url: String,
    pages: RangeInclusive<usize>,
}

impl Pager for HfPager {
    type Context = Context;

    type Item = HfUrl;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        let page = match self.pages.next() {
            Some(page) => page,
            None => return Ok(Page::Empty),
        };
        let url = format!("{}/page/{}", self.base_url, page);
        let document = context.client.get(url).send()?.text()?;
        Ok(Page::Items(context.read_links(&document)))
    }
}

pub struct HfUrl(String);

impl Downloadable for HfUrl {
    type Context = Context;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let page = String::from("https://www.hentai-foundry.com") + &self.0;
        let text = context.client.get(page).send()?.text()?;

        fn extract_by_pattern<'a>(pattern: &Regex, text: &'a str) -> Option<&'a str> {
            Some(pattern.captures(text).map(|x| x.get(1).unwrap())?.as_str())
        }

        let url = extract_by_pattern(&context.image_pattern, &text)
            .or_else(|| extract_by_pattern(&context.full_image_pattern, &text))
            .map(|route| String::from("https://pictures.hentai-foundry.com/") + route)
            .ok_or(Error::Extraction(ExtractionFailure::ImageUrl, self.0))?;

        Ok(context
            .client
            .get(url)
            .send()
            .map(ResponseGalleryItem::new)?)
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

fn build_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .build()
        .unwrap()
}

#[cfg(test)]
mod tests {
    #[test]
    fn can_read_profile_links() {
        assert!(super::extract("https://www.hentai-foundry.com/user/AndavaNSFW").is_ok());
        assert!(super::extract("https://www.hentai-foundry.com/user/AndavaNSFW/profile").is_ok());
    }

    #[test]
    fn can_read_profile_gallery_links() {
        assert!(super::extract("https://www.hentai-foundry.com/pictures/user/AndavaNSFW").is_ok());
    }
}
