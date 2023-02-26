//! Gallery implementation for scrolller.com
//!
//! I'm really only trying to cover individual images at the moment. The way this idiocy works is
//! that they have a GraphQL API that returns a pile of garbage if you just ask nicely based on the
//! URL. GraphQL itself is a fucking retarded API, because I mean... seriously? But it kinda
//! works...

// Test url: https://scrolller.com/whitney-johns-ewhhmc5wuo

use crate::gallery::prelude::*;

use self::data::{Query, Response};

static API_URL: &str = "https://api.scrolller.com/api/v2/graphql";

static QUERY: &str = "query SubredditPostQuery( $url: String! ) { getSubredditPost(url: $url) { \
    id url title fullLengthSource gfycatSource redgifsSource mediaSources { url width height \
        isOptimized } } }";

pub fn extract(url: &str) -> crate::Result<(UnpagedGallery<Image>, Option<String>)> {
    let query = Query::from_url(url);
    let client = Client::builder().user_agent(USER_AGENT).build()?;
    let response: Response = client
        .post(API_URL)
        .json(&query)
        .header("accept", "application/json")
        .send()?
        .json()?;

    Ok((
        UnpagedGallery {
            context: client,
            items: response.urls().map(Image).collect(),
        },
        None,
    ))
}

pub struct Image(String);

impl Downloadable for Image {
    type Context = Client;

    type Output = ResponseGalleryItem;

    fn download(self, context: &Self::Context) -> crate::Result<Self::Output> {
        Ok(ResponseGalleryItem::new(context.get(self.0).send()?))
    }
}

mod data {
    use serde::{Deserialize, Serialize};

    use super::QUERY;

    #[derive(Serialize, Deserialize)]
    pub struct Query<'a> {
        query: &'static str,
        variables: Variables<'a>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Variables<'a> {
        url: &'a str,
    }

    impl<'a> Query<'a> {
        pub fn from_url(url: &'a str) -> Self {
            Self {
                query: QUERY,
                variables: Variables {
                    url: url.strip_prefix("https://scrolller.com").unwrap_or(url),
                },
            }
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct Response {
        data: Data,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Data {
        #[serde(rename = "getSubredditPost")]
        get_subreddit_post: GetSubredditPost,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct GetSubredditPost {
        id: i64,
        title: Option<String>,
        full_length_source: Option<String>,
        gfycat_source: Option<String>,
        redgifs_source: Option<String>,
        media_sources: Vec<MediaSource>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MediaSource {
        url: String,
        width: Option<i64>,
        height: Option<i64>,
        #[serde(default)]
        is_optimized: bool,
    }

    impl Response {
        pub fn urls(&self) -> impl Iterator<Item = String> + '_ {
            struct RepsonseIter<'a> {
                idx: u8,
                source: &'a Response,
            }

            impl<'a> RepsonseIter<'a> {
                fn new(response: &'a Response) -> Self {
                    RepsonseIter {
                        idx: 0,
                        source: response,
                    }
                }
            }

            impl<'a> Iterator for RepsonseIter<'a> {
                type Item = String;

                fn next(&mut self) -> Option<Self::Item> {
                    if self.idx == 3 {
                        return None;
                    }

                    self.idx += 1;

                    match self.idx {
                        1 => self
                            .source
                            .data
                            .get_subreddit_post
                            .full_length_source
                            .as_deref()
                            .map(|s| s.into()),

                        2 => self
                            .source
                            .data
                            .get_subreddit_post
                            .gfycat_source
                            .as_deref()
                            .map(|s| s.into()),

                        3 => self
                            .source
                            .data
                            .get_subreddit_post
                            .redgifs_source
                            .as_deref()
                            .map(|s| s.into()),

                        _ => unreachable!("We have an early return for this."),
                    }
                }
            }

            self.data
                .get_subreddit_post
                .media_sources
                .iter()
                .map(|s| s.url.clone())
                .chain(RepsonseIter::new(self))
        }
    }
}
