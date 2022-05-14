use std::{borrow::Cow, io, path::PathBuf};

use clap::Parser;

use crate::storage::StorageProvider;

/// A program for downloading image galleries.
///
/// It's best not to pass in your username and password. Instead, feel free to include that in
/// a .env file when the program is compiled.
#[derive(Clone, Debug, Parser)]
pub struct Opt {
    /// The target url.
    url: String,

    /// A directory for new files.
    directory: Option<String>,

    /// A base name to be used in naming downloaded files.
    #[clap(short, long = "name")]
    name_override: Option<String>,

    /// Auto-derive name
    ///
    /// Instructs imgrab to automatically derive a gallery name from the gallery url when
    /// possible. In general, this is possible for galleries by an artist or of a model;
    /// galleries based on tag searches will generally not provide an auto-name.
    ///
    /// In the event a name cannot be derived, the base name can be used as a fallback, or
    /// else the download will fail.
    #[clap(short, long = "auto")]
    auto_name: bool,

    /// Add a cooldown between image downloads.
    #[clap(short, long)]
    wait: Option<u64>,

    /// Overwrite existing files.
    #[clap(short, long)]
    overwrite: bool,

    /// Skip n images.
    #[clap(short, long)]
    pub skip: Option<usize>,

    /// Take n images.
    #[clap(short, long)]
    pub take: Option<usize>,
}

impl Opt {
    pub fn parse() -> Self {
        Parser::parse()
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn wait(&self) -> Option<u64> {
        self.wait
    }

    pub fn overwrite(&self) -> bool {
        self.overwrite
    }

    pub fn storage_provider(
        &self,
        current_dir: impl Into<PathBuf>,
        gallery_name: Option<&str>,
    ) -> crate::Result<StorageProvider> {
        use std::fs;

        let Opt {
            directory,
            name_override,
            ..
        } = self;

        let directory = gallery_name
            .map(make_safe_name)
            .or_else(|| directory.as_deref().map(Cow::from));

        // It is an error for the user to request an auto name and for us to have no name to use.
        if self.auto_name && directory.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "auto name not available; use name override",
            )
            .into());
        }

        let mut current_dir = current_dir.into();

        // This directory logic is a little convoluted. In short, if the user has provided
        // an existing path, roll with it. Otherwise, use the current directory with their
        // provided string appended to the end.
        let path = match directory {
            Some(path) => match fs::canonicalize(&*path) {
                Ok(path) => path,
                _ => {
                    current_dir.push(&*path);
                    fs::create_dir(&current_dir)?;
                    current_dir
                }
            },

            None => current_dir,
        };

        Ok(StorageProvider::new(path, name_override.clone()))
    }
}

fn make_safe_name(name: &str) -> Cow<str> {
    for (idx, u) in name.bytes().enumerate() {
        if is_illegal_char(u) {
            return Cow::from(build_filtered_string(
                &name[..idx],
                &name[idx + 1..],
                u == b'%',
            ));
        }
    }
    Cow::from(name)
}

fn build_filtered_string(head: &str, tail: &str, mut skip_numerals: bool) -> String {
    let mut has_invalid_char = false;
    let mut buf = String::with_capacity(head.len() + tail.len() + 1);

    buf.push_str(head);
    buf.push('_');

    for u in tail.bytes() {
        if is_illegal_char(u) {
            if u == b'%' {
                skip_numerals = true;
            }

            if !has_invalid_char {
                has_invalid_char = true;
                buf.push('_');
            }
        } else {
            if skip_numerals && matches!(u, b'0'..=b'9') {
                continue;
            }

            buf.push(u as char);
            if has_invalid_char {
                has_invalid_char = false;
            }

            if skip_numerals {
                skip_numerals = false;
            }
        }
    }

    buf
}

fn is_illegal_char(u: u8) -> bool {
    !u.is_ascii() || !matches!(u, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b' ')
}
