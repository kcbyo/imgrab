use regex::Regex;

use crate::gallery::prelude::*;

pub struct Sankaku;

impl ReadGallery for Sankaku {
    fn read(self, url: &str) -> crate::Result<DynamicGallery> {
        // First thing first, we don't actually want to use the url as provided. We want to
        // transpile (ha) that url into something that targets Sankaku's wonderfully entertaining
        // paging api. To do that, we effectively just need the tags, but I'm still thinking about
        // just how to get those...
        //
        // Sample URL: https://chan.sankakucomplex.com/?tags=korra+sweat+abs&commit=Search

        // If we can't extract some tags, this url is no good. There's no point scraping all of
        // Sankaku Complex. No one needs that much pr0n. GO OUTSIDE. Once we have these, we can
        // construct a request to the paging api instead in order to snag links from that.
        let tags = extract_tags(url)
            .ok_or_else(|| Error::Unsupported(UnsupportedError::Route, url.into()))?;
        let url = format!(
            "https://chan.sankakucomplex.com/post/index.content?tags={}",
            tags
        );

        let client = build_client()?;
        let page = client.get(&url).send()?.text()?;

        let next_page_url_pattern = Regex::new(r#"next-page-url="([^"]+)""#).unwrap();
        let image_post_url_pattern = Regex::new(r#"a href="/post/show/(\d+)""#).unwrap();
        let image_pattern = Regex::new(r#"href="(.+?)" id=highres"#).unwrap();

        // Important to remember: it's perfectly feasible that this will not exist. Short search
        // result sets will not have a next page, and neither will the last page of results.
        let next_url = next_page_url_pattern
            .captures(&page)
            .map(|capture| capture.get(1).unwrap().as_str().into());

        let queue = image_post_url_pattern
            .captures_iter(&page)
            .map(|capture| capture.get(1).unwrap().as_str().to_owned())
            .collect();

        Ok(Box::new(SankakuGallery {
            client,
            next_url,
            queue,
            next_page_url_pattern,
            image_post_url_pattern,
            image_pattern,
        }))
    }
}

pub struct SankakuGallery {
    client: Client,
    next_url: Option<String>,
    queue: VecDeque<String>,
    next_page_url_pattern: Regex,
    image_post_url_pattern: Regex,
    image_pattern: Regex,
}

impl SankakuGallery {
    fn retrieve_batch(&mut self) -> crate::Result<()> {
        // This:
        // > /?next=3539444&amp;tags=korra%20slave&amp;page=4
        //
        // Needs to be turned into this:
        // > https://chan.sankakucomplex.com/post/index.content?next=3539444&tags=korra%20slave&page=4
        //
        // No clue why this dumb thing has a slash on the front of it. I guess we'll just pull
        // that off before we build the finished url.

        let url = match self.next_url.take() {
            Some(url) => format!(
                "https://chan.sankakucomplex.com/post/index.content{}",
                url.trim_start_matches('/')
            ),
            None => return Ok(()),
        };

        let page_content = self.client.get(&url).send()?.text()?;

        self.next_url = self
            .next_page_url_pattern
            .captures(&page_content)
            .map(|capture| capture.get(1).unwrap().as_str().into());

        self.queue = self
            .image_post_url_pattern
            .captures_iter(&page_content)
            .map(|capture| capture.get(1).unwrap().as_str().to_owned())
            .collect();

        Ok(())
    }

    fn retrieve_image_url(&self, url: &str) -> crate::Result<String> {
        let page_content = self.client.get(url).send()?.text()?;
        self.image_pattern
            .captures(&page_content)
            .and_then(|x| x.get(1).map(|x| x.as_str().replace("&amp;", "&")))
            .ok_or_else(|| Error::Extraction(ExtractionFailure::ImageUrl, url.into()))
    }
}

impl Gallery for SankakuGallery {
    fn apply_skip(&mut self, mut skip: usize) -> crate::Result<()> {
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

            if self.next_url.is_some() {
                self.retrieve_batch()?;
            }
        }
    }
}

impl Iterator for SankakuGallery {
    type Item = crate::Result<GalleryItem>;

    fn next(&mut self) -> Option<Self::Item> {
        // Because of the relative simplicity of this api, it's possible for us to infer the
        // state of the iterator. Of course, inferring something like that results in marginally
        // annoying branching... In this case, if the queue is empty and we have no next url, we
        // call it quits. If the queue is empty but we do have a next url, we attempt to refill
        // the queue. Assuming there's anything to put into the queue, we continue iteration.
        if self.queue.is_empty() {
            self.next_url.as_ref()?;
            if let Err(e) = self.retrieve_batch() {
                return Some(Err(e));
            }
        }

        // The full url should look like this:
        // https://chan.sankakucomplex.com/post/show/3523419

        let post_url = self
            .queue
            .pop_front()
            .map(|post| format!("https://chan.sankakucomplex.com/post/show/{}", post))?;

        // Note that the urls being used by the website are protocol-relative.
        let image_url = match self.retrieve_image_url(&post_url) {
            Ok(url) => format!("https:{}", url),
            Err(e) => return Some(Err(e)),
        };

        match self.client.get(&image_url).send() {
            Ok(response) => Some(Ok(GalleryItem::new(image_url, response))),
            Err(e) => Some(Err(e.into())),
        }
    }
}

fn extract_tags(url: &str) -> Option<&str> {
    let pattern = Regex::new(r#"tags=([^&]+)"#).unwrap();
    let capture = pattern.captures(url)?;
    capture.get(1).map(|x| x.as_str())
}

fn build_client() -> crate::Result<Client> {
    use reqwest::header;

    let builder = Client::builder()
        .user_agent(super::USER_AGENT)
        .cookie_store(true);
    let mut headers = header::HeaderMap::new();

    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("text/html"),
    );

    Ok(builder.default_headers(headers).build()?)
}
