use regex::Regex;

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<(PagedGallery<GwmPager>, Option<String>)> {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("text/html"));

    let context = Context {
        client: Client::builder()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            .build()
            .unwrap(),
        image_id_pattern: Regex::new(r#"imgid(\d+)"#).unwrap(),
        data_url_pattern: Regex::new(r#"images/full/\d+\.[^"]+"#).unwrap(),
    };

    let gallery = PagedGallery {
        context,
        current: Page::Empty,
        pager: GwmPager {
            name: read_name(url)?.into(),
            page: 1,
            previous_items: VecDeque::new(),
        },
    };

    Ok((gallery, None))
}

pub struct Context {
    client: Client,
    image_id_pattern: Regex,
    data_url_pattern: Regex,
}

impl Context {
    fn get_page_content(&self, url: &str) -> crate::Result<String> {
        Ok(self.client.get(url).send()?.text()?)
    }

    fn get_full_image_link(&self, id: &str) -> crate::Result<String> {
        let url = format!("https://www.girlswithmuscle.com/{}/", id);
        let content = self.client.get(&url).send()?.text()?;
        let data_url = self
            .data_url_pattern
            .captures(&content)
            .ok_or_else(|| Error::Extraction(ExtractionFailure::ImageUrl, url.clone()))?;
        Ok(String::from("https://www.girlswithmuscle.com/") + data_url.get(0).unwrap().as_str())
    }
}

pub struct GwmPager {
    name: String,
    page: usize,
    previous_items: VecDeque<Id>,
}

impl Pager for GwmPager {
    type Context = Context;

    type Item = Id;

    fn next_page(&mut self, context: &Self::Context) -> crate::Result<Page<Self::Item>> {
        let url = format!(
            "https://www.girlswithmuscle.com/images/{}/?name={}",
            self.page, self.name,
        );
        let text = context.get_page_content(&url)?;
        let items: VecDeque<_> = context
            .image_id_pattern
            .captures_iter(&text)
            .map(|capture| Id(capture.get(1).unwrap().as_str().into()))
            .collect();

        // This Simple Simon-ass API continues returning the last page of results as many times
        // as you'd care to fetch it, thereby repeating the last N images in your downloads folder
        // until either A) your drive fills up or B) your ISP kicks you offline.
        //
        // Rather than waste time and bandwidth on this stupidity, we'll check to see if we've
        // collected the same IDs over again before returning the result.
        //
        // No, it's not efficient. No, I don't care.

        self.page += 1;
        if items == self.previous_items {
            Ok(Page::Empty)
        } else {
            self.previous_items = items.clone();
            Ok(Page::Items(items))
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Id(String);

impl Downloadable for Id {
    type Context = Context;
    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let url = context.get_full_image_link(&self.0)?;
        Ok(context
            .client
            .get(url)
            .send()
            .map(ResponseGalleryItem::new)?)
    }
}

fn read_name(url: &str) -> crate::Result<&str> {
    let pattern = Regex::new(r#"name=([^&]+)"#).unwrap();
    Ok(pattern
        .captures(url)
        .ok_or_else(|| Error::Unsupported(UnsupportedError::Route, url.into()))?
        .get(1)
        .unwrap()
        .as_str())
}
