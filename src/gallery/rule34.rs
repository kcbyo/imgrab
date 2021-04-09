use super::prelude::*;
use crate::tags::Tags;

pub fn extract(url: &str) -> crate::Result<Rule34Gallery> {
    let url = url.trim_end_matches('#');
    let tags = Tags::try_from_url(url, "+").ok_or_else(|| {
        Error::Unsupported(
            UnsupportedError::Route,
            String::from("Rule34 urls must have one or more tags"),
        )
    })?;

    Ok(Rule34Gallery::new(tags))
}

pub struct Rule34Gallery {
    client: Client,
    tags: Tags,
    count: usize,
    queue: VecDeque<String>,

    // Patterns
    image_page_pattern: Regex,
    image_url_pattern: Regex,

    /// is_complete is set to true when a fill call succeeds with zero results.
    is_complete: bool,
}

impl Rule34Gallery {
    fn new(tags: Tags) -> Self {
        let image_page_pattern =
            Regex::new(r#"href="(index.php\?page=post&s=view&id=\d+)""#).unwrap();
        let image_url_pattern =
            Regex::new(r#"<meta property="og:image" itemprop="image" content="([^"]+)""#).unwrap();

        Rule34Gallery {
            client: Client::builder().build().unwrap(),
            tags,
            count: 0,
            queue: VecDeque::new(),
            image_page_pattern,
            image_url_pattern,
            is_complete: false,
        }
    }

    fn retrieve_batch(&mut self) -> crate::Result<usize> {
        let url = format!(
            "https://rule34.xxx/index.php?page=post&s=list&tags={}&pid={}",
            self.tags, self.count
        );

        let page_content = self.client.get(&url).send()?.text()?;
        let all_matches = self.image_page_pattern.captures_iter(&page_content);

        for capture in all_matches {
            let relative_url = capture.get(1).unwrap().as_str();

            // FIXME: the relative_url.replace() call below is probably no longer necessary.
            self.queue
                .push_back("https://rule34.xxx/".to_owned() + &relative_url.replace("&amp;", "&"));
        }

        Ok(self.queue.len())
    }

    fn retrieve_image_url(&self, url: &str) -> crate::Result<String> {
        let page_content = self.client.get(url).send()?.text()?;
        let captures = self
            .image_url_pattern
            .captures(&page_content)
            .ok_or_else(|| Error::Extraction(ExtractionFailure::ImageUrl, url.into()))?;

        Ok(captures.get(1).unwrap().as_str().into())
    }
}

impl Gallery for Rule34Gallery {
    fn advance_by(&mut self, mut skip: usize) -> crate::Result<()> {
        loop {
            if skip == 0 {
                return Ok(());
            }

            if skip < self.queue.len() {
                self.queue.drain(..skip);
                return Ok(());
            }

            skip = skip.saturating_sub(self.queue.len());
            self.queue.clear();
            match self.retrieve_batch()? {
                0 => {
                    self.is_complete = true;
                    return Ok(());
                }
                count => self.count += count,
            }
        }
    }

    fn next(&mut self) -> Option<crate::Result<GalleryItem>> {
        // This process is familiar to me at this point, but the basic plan is as follows: grab a
        // set of links from the main page and store those in a fifo queue. On each subsequent
        // call to the iterator, either pop an item off the queue or refill the queue and then
        // attempt to pop an item. If the queue is ever empty and cannot be refilled, iteration
        // is complete.

        // The only wrinkle is that this iterator must also perform the request to get the image
        // data, because what this iterator is meant to return is A) the final url, and B) the
        // actual response stream containing the image itself.

        if self.is_complete {
            return None;
        }

        if self.queue.is_empty() {
            match self.retrieve_batch() {
                Ok(0) => {
                    self.is_complete = true;
                    return None;
                }
                Ok(count) => self.count += count,
                Err(e) => return Some(Err(e)),
            }
        }

        // With my other implementations, this would be the final step. However, because this one
        // needs to return a response from the server rather than simply the url, I need to pop
        // the item from the queue, fetch the image url, and make the request.

        let image_page = self.queue.pop_front()?;
        let image_url = match self.retrieve_image_url(&image_page) {
            Ok(url) => url,
            Err(e) => return Some(Err(e)),
        };

        match self.client.get(&image_url).send() {
            Ok(response) => Some(Ok(GalleryItem::new(image_url, response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}
