use scraper::{Html, Selector};

use crate::gallery::prelude::*;

pub fn extract(url: &str) -> crate::Result<(UnpagedGallery<ImageLink>, Option<String>)> {
    let client = Client::builder().user_agent(USER_AGENT).build()?;

    let title_s = Selector::parse("title").unwrap();
    let image_s = Selector::parse("div.reading-content img[data-src]").unwrap();

    let text = client.get(url).send()?.text()?;
    let document = Html::parse_fragment(&text);

    // This title business looks complicated but isn't.

    let title = document
        .select(&title_s)
        .next()
        .map(|x| x.text().collect())
        .map(|title: String| {
            title
                .strip_suffix(" - NovelCrow")
                .map(|title| title.to_owned())
                .unwrap_or(title)
        });

    let images = document
        .select(&image_s)
        .filter_map(|x| x.value().attr("data-src"))
        .map(|x| x.into())
        .collect();

    Ok((
        UnpagedGallery {
            context: client,
            items: images,
        },
        title,
    ))
}
