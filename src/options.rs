use std::{io, path::PathBuf};

use structopt::StructOpt;

use crate::storage::StorageProvider;

/// A program for downloading image galleries.
///
/// It's best not to pass in your username and password. Instead, feel free to include that in
/// a .env file when the program is compiled.
#[derive(Debug, StructOpt)]
pub struct Opt {
    /// The target url.
    url: String,

    /// A directory for new files.
    // #[structopt(short = "d", long = "dir", parse(from_os_str))]
    directory: Option<String>,

    /// A base name to be used in naming downloaded files.
    #[structopt(short = "n", long = "name")]
    name_override: Option<String>,

    /// Auto-derive name
    ///
    /// Instructs imgrab to automatically derive a gallery name from the gallery url when
    /// possible. In general, this is possible for galleries by an artist or of a model;
    /// galleries based on tag searches will generally not provide an auto-name.
    ///
    /// In the event a name cannot be derived, the base name can be used as a fallback, or
    /// else the download will fail.
    #[structopt(short, long = "auto")]
    auto_name: bool,

    /// Add a cooldown between image downloads.
    #[structopt(short = "w", long = "wait")]
    wait: Option<u64>,

    /// Overwrite existing files.
    #[structopt(short, long)]
    overwrite: bool,

    /// Skip n images.
    #[structopt(short, long)]
    pub skip: Option<usize>,

    /// Take n images.
    #[structopt(short, long)]
    pub take: Option<usize>,
}

impl Opt {
    pub fn from_args() -> Self {
        StructOpt::from_args()
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

        let name_override = gallery_name
            .filter(|_| self.auto_name)
            .or_else(|| name_override.as_ref().map(AsRef::as_ref));

        // It is an error for the user to request an auto name and for us to have no name to use.
        if self.auto_name && name_override.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "autoname unavailable; use name override",
            )
            .into());
        }

        let mut current_dir = current_dir.into();

        // This directory logic is a little convoluted. In short, if the user has provided
        // an existing path, roll with it. Otherwise, use the current directory with their
        // provided string appended to the end.
        let path = match directory {
            Some(path) => match fs::canonicalize(&path) {
                Ok(path) => path,
                _ => {
                    current_dir.push(path);
                    fs::create_dir(&current_dir)?;
                    current_dir
                }
            },

            None => current_dir,
        };

        Ok(StorageProvider::new(
            path,
            name_override.map(|name| name.to_string()),
        ))
    }
}
