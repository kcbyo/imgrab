use regex::Regex;

use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<PagedGallery<GwmPager>> {
    let context = Context {
        agent: AgentBuilder::new().user_agent(USER_AGENT).build(),
        image_id_pattern: Regex::new(r#"imgid(\d+)"#).unwrap(),
        data_url_pattern: Regex::new(r#"images/full/\d+\.[^"]+"#).unwrap(),
    };

    Ok(PagedGallery {
        context,
        current_page: VecDeque::new(),
        pager: GwmPager {
            name: read_name(url)?.into(),
            page: 1,
            previous_items: VecDeque::new(),
        }
    })
}

pub struct Context {
    agent: Agent,
    image_id_pattern: Regex,
    data_url_pattern: Regex,
}

impl Context {
    fn get(&self, url: &str) -> UreqResponse {
        self.agent.get(url).set("Accept", "text/html").call()
    }

    fn get_page_content(&self, url: &str) -> crate::Result<String> {
        Ok(self.get(&url)?.into_string()?)
    }

    fn get_full_image_link(&self, id: &str) -> crate::Result<String> {
        let url = format!("https://www.girlswithmuscle.com/{}/", id);
        let content = self.get(&url)?.into_string()?;
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

    fn next_page(&mut self, context: &Self::Context) -> Option<crate::Result<VecDeque<Self::Item>>> {
        let url = format!(
            "https://www.girlswithmuscle.com/images/{}/?name={}",
            self.page, self.name,
        );
        let text = match context.get_page_content(&url) {
            Ok(text) => text,
            Err(e) => return Some(Err(e)),
        };
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
            None
        } else {
            self.previous_items = items.clone();
            Some(Ok(items))
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Id(String);

impl Downloadable for Id {
    type Context = Context;
    type Output = UreqGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        let url = context.get_full_image_link(&self.0)?;
        Ok(context.get(&url).map(UreqGalleryItem::new)?)
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
