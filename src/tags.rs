use std::{collections::HashMap, fmt::Display};

// Had an issue earlier with the following url:
// https://rule34.xxx/index.php?page=post&s=list&tags=tekuho+#
//
// The problem is that the application was unable to deal with the trailing +#
// found on this particular (spurious) url; it continually received the first
// page of results in response to its queries.
#[derive(Clone, Debug)]
pub struct Tags {
    sep: String,
    values: Vec<String>,
}

impl Tags {
    pub fn try_from_url(url: &str, split: &str) -> Option<Self> {
        let parameters = url.find('?').map(|idx| &url[idx + 1..])?;
        let parameters = parameters
            .rfind('#')
            .map(|idx| &parameters[..idx])
            .unwrap_or(parameters);
        let parameters: HashMap<_, _> = parameters
            .split('&')
            .filter_map(|segment| segment.find('=').map(|mid| segment.split_at(mid)))
            .collect();

        parameters.get("tags").map(|&tags| Self {
            sep: split.to_string(),
            values: tags
                .split(split)
                .map(|tag| tag.trim_matches(|u: char| !u.is_alphanumeric()).to_string())
                .collect(),
        })
    }
}

impl Display for Tags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut tags = self.values.iter();

        if let Some(tag) = tags.next() {
            f.write_str(tag)?;
        }

        for tag in tags {
            write!(f, "{}{}", self.sep, tag)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Tags;

    #[test]
    fn can_extract_tags() {
        let url = "https://foo.bar.com/?tags=one%20two%20three";
        let tags = Tags::try_from_url(url, "%20").unwrap();
        assert_eq!(&tags.values, &["one", "two", "three"])
    }
}
