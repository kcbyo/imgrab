use super::prelude::*;

pub fn extract(url: &str) -> crate::Result<GwmGallery> {
    let name = read_name(url)?.into();
    Ok(GwmGallery {
        client: build_client()?,
        name,
        page: 1,
        image_id_pattern: Regex::new(r#"imgid(\d+)"#).unwrap(),
        data_url_pattern: Regex::new(r#"images/full/\d+\.[^"]+"#).unwrap(),
        queue: VecDeque::new(),
        last_queue: VecDeque::new(),
        is_complete: false,
        skip: 0,
    })
}

pub struct GwmGallery {
    client: Client,
    name: String,
    image_id_pattern: Regex,
    data_url_pattern: Regex,
    page: usize,
    queue: VecDeque<String>,
    last_queue: VecDeque<String>,
    is_complete: bool,
    skip: usize,
}

impl GwmGallery {
    fn retrieve_batch(&mut self) -> crate::Result<usize> {
        let url = format!(
            "https://www.girlswithmuscle.com/images/{}/?name={}",
            self.page, self.name,
        );
        let response = self.client.get(&url).send()?.text()?;
        let next_queue = self
            .image_id_pattern
            .captures_iter(&response)
            .map(|capture| capture.get(1).unwrap().as_str().into())
            .collect();

        // This Simple Simon-ass API continues returning the last page of results as many times
        // as you'd care to fetch it, thereby repeating the last N images in your downloads folder
        // until either A) your drive fills up or B) your ISP kicks you offline.
        //
        // Rather than waste time and bandwidth on this stupidity, we'll check to see if we've
        // collected the same IDs over again before returning the result.
        //
        // No, it's not efficient. No, I don't care.

        if self.last_queue == next_queue {
            Ok(0)
        } else {
            self.queue = next_queue;
            self.last_queue = self.queue.clone();
            Ok(self.queue.len())
        }
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

impl Gallery for GwmGallery {
    fn advance_by(&mut self, skip: usize) -> crate::Result<()> {
        self.skip = skip;
        Ok(())
    }

    fn next(&mut self) -> Option<crate::Result<GalleryItem>> {
        if self.is_complete {
            return None;
        }

        if self.skip > 0 {
            if self.skip > self.queue.len() {
                self.skip -= self.queue.len();
                self.queue.clear();
            } else {
                self.skip = 0;
                self.queue.drain(..self.skip);
            }
        }

        if self.queue.is_empty() {
            match self.retrieve_batch() {
                Ok(0) => {
                    self.is_complete = true;
                    return None;
                }

                Err(e) => {
                    self.is_complete = true;
                    return Some(Err(e));
                }

                _ => self.page += 1,
            }
        }

        let id = self.queue.pop_front()?;
        match self.get_full_image_link(&id).and_then(|url| {
            self.client
                .get(&url)
                .send()
                .map_err(|e| e.into())
                .map(|response| (url, response))
        }) {
            Ok((url, response)) => Some(Ok(GalleryItem::new(url, response))),
            Err(e) => Some(Err(e)),
        }
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

fn build_client() -> crate::Result<Client> {
    use reqwest::header;

    let builder = Client::builder();
    let mut headers = header::HeaderMap::new();

    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("text/html"),
    );
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(super::USER_AGENT),
    );

    Ok(builder.default_headers(headers).build()?)
}
