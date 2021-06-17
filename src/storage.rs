use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct StorageProvider {
    path: PathBuf,
    count: usize,
    name_override: Option<String>,
}

impl StorageProvider {
    pub fn new(path: impl Into<PathBuf>, name_override: Option<String>) -> Self {
        Self {
            path: path.into(),
            count: 0,
            name_override,
        }
    }

    pub fn create_path(&mut self, context: NameContext) -> PathBuf {
        // Our name process may be a little branchy, but it's been abstracted a bit...
        //
        // If the user has provided an override, use the override in conjunction with a counter.
        // Given we have no idea how many files are in a given gallery, this may be less than
        // ideal, but most will have fewer than a thousand, so... Meh?
        //
        // If the user has not provided an override, we'll use either the name provided by the
        // gallery *or* the final segment of the url, which should somewhat resemble a filename.
        //
        // As a final fallback, we'll just use the count as a name, but that seems unlikely.

        self.count += 1;
        let name = match self.name_override.as_ref() {
            Some(name) => Cow::from(format!("{}{:03}", name, self.count)),
            None => context
                .name()
                .map(Cow::from)
                .unwrap_or_else(|| Cow::from(format!("{:03}", self.count))),
        };

        self.path.join(&*name)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub struct NameContext<'item> {
    url: &'item str,
    name: Option<Cow<'item, str>>,
}

impl<'a> NameContext<'a> {
    pub fn new(url: &'a str, name: Option<Cow<'a, str>>) -> Self {
        NameContext { url, name }
    }

    pub fn from_response(response: &'a ureq::Response) -> Self {
        static CONTENT_DISPOSITION: &str = "Content-Disposition";

        let name = response.header(CONTENT_DISPOSITION).and_then(read_filename);

        NameContext::new(response.get_url(), name.map(Cow::from))
    }

    /// Gets the best name from the gallery.
    ///
    /// This name may be the final segment of the URL, or it may be a more descriptive name
    /// provided by some other means.
    fn name(&self) -> Option<&str> {
        self.name
            .as_ref()
            .map(AsRef::as_ref)
            .or_else(|| name_from_url(self.url))
    }
}

fn name_from_url(s: &str) -> Option<&str> {
    // Urls may have parameters, e.g. ?timestamp=2
    // We need to eliminate these as well.

    if s.ends_with('/') {
        return None;
    }

    let s = match s.rfind('/') {
        Some(idx) => &s[(idx + 1)..],
        None => s,
    };

    let s = match s.rfind('?') {
        Some(idx) => &s[..idx],
        None => s,
    };

    if s.is_empty() {
        return None;
    }

    // If we're still working, we just need to be sure we don't hand back a URL
    // containing a useless filename like fullimg.php
    let extension = match s.rfind('.') {
        Some(idx) => &s[idx..],
        None => return Some(s),
    };

    match extension {
        ".php" | ".html" => None,
        _ => Some(s),
    }
}

fn read_filename(disposition: &str) -> Option<String> {
    // "content-disposition": "attachment; filename=114_Turtlechan_312677_FISHOOKERS_PAGE_3.png"
    disposition
        .rfind("filename=")
        .map(|idx| disposition[(idx + 9)..].to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_from_url() {
        // Sam Gardner, barbarian mode, chained to the wall, covered in cum.
        let url = "https://us.rule34.xxx//images/2867/ae0e7f9b7a31e3f04db0de05bddfb2ce.png";
        let expected = Some("ae0e7f9b7a31e3f04db0de05bddfb2ce.png");
        assert_eq!(expected, super::name_from_url(url));

        // Sam Gardner's directory
        let url = "https://us.rule34.xxx//images/2867/";
        assert_eq!(None, super::name_from_url(url));

        // Korra's a slut
        let url = "https://cs.sankakucomplex.com/data/sample/83/2b/sample-832bebf845e9107b596b5037c951691c.jpg?e=1562128678&m=BxhNLAnQgUm0I0pBWlonxA";
        let expected = Some("sample-832bebf845e9107b596b5037c951691c.jpg");
        assert_eq!(expected, super::name_from_url(url));
    }
}
