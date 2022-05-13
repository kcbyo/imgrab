use std::{io, path::PathBuf};

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

        let directory = gallery_name.or(directory.as_deref());

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

        Ok(StorageProvider::new(path, name_override.clone()))
    }
}
