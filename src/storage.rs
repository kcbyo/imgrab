use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
};

use reqwest::blocking::Response;

#[derive(Debug)]
pub struct StorageProvider {
    path: PathBuf,
    count: usize,
    filter: HashMap<String, usize>,
    name_override: Option<String>,
}

impl StorageProvider {
    pub fn new(path: impl Into<PathBuf>, name_override: Option<String>) -> Self {
        Self {
            path: path.into(),
            count: 0,
            filter: HashMap::new(),
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

        // Here we check to see how many files we've downloaded using this same name. If the number
        // isn't zero (it should be), we add ($nth) to the end of the name. Because the name has a
        // filename extension on it already by this point, this will be fucking annoying.

        let nth = self.get_path_count(&name);
        let path = self.path.join(&*name);

        if nth > 0 {
            nth_path(&path, nth)
        } else {
            path
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn get_path_count(&mut self, name: &str) -> usize {
        let entry = self.filter.entry(name.to_string()).or_default();
        let result = *entry;
        *entry += 1;
        result
    }
}

fn nth_path(path: &Path, nth: usize) -> PathBuf {
    // Rust does some stupid things when it comes to identifying filename extensions. For instance,
    // the standard library thinks foo.tar.gz has a file stem of foo and an extension of .gz. For
    // pretty obvious reasons, this is not acceptable. Also, we need to do some serious processing
    // on this string, so we're going to make it an actual string. None of this path crap.

    let name = path.file_name().unwrap().to_string_lossy();

    // Our new and improved definition for a filename extension is "any uninterrupted series of
    // characters such that we alternate from ascii alphabetic characters to dots." .xx.yy.zzz
    // is a perfectly valid extension. .1080p.mp4 is not. We're going to split on '.' and go from
    // there. The first segment will indubitably be the filename, but the second will (in most
    // cases) be the extension, or at least a part of it. Where this really breaks down is in
    // torrented filenames, which can look like "The.Fate.of.the.Furious.wtf.720p.XXL337XX.mp4"

    // Actually, I think I'm just gonna use a list. We're just gonna roll with the extension being
    // all the consecutive extension-like substrings at the end of our filename.

    static COMMON_EXTENSIONS: &[&str] = &[
        "7zip", "aac", "accdb", "accde", "accdr", "accdt", "adt", "adts", "aif", "aifc", "aiff",
        "aspx", "avi", "bak", "bat", "bin", "bmp", "cab", "cda", "csv", "dif", "dll", "doc",
        "docm", "docx", "dot", "dotx", "eml", "eps", "exe", "flv", "gif", "gz", "htm", "html",
        "ini", "iso", "jar", "jpeg", "jpg", "m4a", "mdb", "mid", "midi", "mov", "mp3", "mp4",
        "mp4", "mpeg", "mpg", "msi", "mui", "pdf", "png", "pot", "potm", "potx", "ppam", "pps",
        "ppsm", "ppsx", "ppt", "pptm", "pptx", "psd", "pst", "pub", "rar", "rtf", "sldm", "sldx",
        "swf", "sys", "tar", "tif", "tiff", "tmp", "txt", "vob", "vsd", "vsdm", "vsdx", "vss",
        "vssm", "vst", "vstm", "vstx", "wav", "wbk", "wks", "wma", "wmd", "wms", "wmv", "wmz",
        "wp5", "wpd", "xla", "xlam", "xll", "xlm", "xls", "xlsm", "xlsx", "xlt", "xltm", "xltx",
        "xps", "zip",
    ];

    // Note that these are in order from right to left, not left to right.
    let extension_segments: Vec<_> = name
        .rsplit('.')
        .take_while(|segment| {
            // Could probably just use a hashset or something, but binary search means I don't need
            // to allocate anything.
            COMMON_EXTENSIONS.binary_search(segment).is_ok()
                || COMMON_EXTENSIONS
                    .binary_search(&segment.to_ascii_lowercase().as_ref())
                    .is_ok()
        })
        .collect();

    // Before we get deeper, let's be sure the file name even *has* an extension. If not....
    if path.extension().is_none() || extension_segments.is_empty() {
        return format!("{} ({})", name, nth).into();
    }

    let first_extension_segment = *extension_segments.last().unwrap();
    let extension_start = name.find(first_extension_segment).unwrap() - 1;
    let stem = &name[..extension_start];

    // If we have no file stem remaining, something has gone terribly, terribly wrong.
    if stem.is_empty() {
        return format!("{}.{}", name, nth).into();
    }

    // Now that we have the file stem, we combine that with $nth and with the extension we derived
    // above and return it as our shiny new filename.

    let mut extension = String::from(*extension_segments.last().unwrap());
    extension_segments[..extension_segments.len() - 1]
        .iter()
        .rev()
        .for_each(|&segment| {
            extension += ".";
            extension += segment;
        });

    let name = format!("{} ({}).{}", stem, nth, extension);
    path.with_file_name(name)
}

#[derive(Clone, Debug)]
pub struct NameContext<'item> {
    url: &'item str,
    name: Option<Cow<'item, str>>,
}

impl<'a> NameContext<'a> {
    pub fn new(url: &'a str, name: Option<Cow<'a, str>>) -> Self {
        NameContext { url, name }
    }

    pub fn from_response(response: &'a Response) -> Self {
        use reqwest::header::CONTENT_DISPOSITION;
        let name = response
            .headers()
            .get(CONTENT_DISPOSITION)
            .and_then(|header| header.to_str().ok().and_then(read_filename));
        NameContext::new(response.url().as_ref(), name.map(Cow::from))
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
    use std::path::Path;

    use super::{NameContext, StorageProvider};

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

    #[test]
    fn nth_path() {
        let cases = &[
            (Path::new("foo.jpg"), Path::new("foo (1).jpg")),
            (Path::new("foo.tar.gz"), Path::new("foo (1).tar.gz")),
        ];

        for &(given, expected) in cases {
            assert_eq!(super::nth_path(given, 1), expected);
        }
    }

    #[test]
    fn storage_provider_data_protection() {
        let mut provider = StorageProvider::new("/", None);
        let context = NameContext::new("https://foo.com/bar.jpg", None);

        let a = dbg!(provider.create_path(context.clone()));
        let b = dbg!(provider.create_path(context));

        assert_eq!(a, Path::new("/bar.jpg"));
        assert_eq!(b, Path::new("/bar (1).jpg"));
    }
}
