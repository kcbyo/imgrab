use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    path::{Path, PathBuf},
};

mod config;
mod error;
mod format;
mod gallery;
mod options;
mod storage;
mod tags;
mod waiter;

use error::{Error, UnsupportedError};
use fmtsize::{Conventional, FmtSize};
use format::DurationFormat;
use gallery::Gallery;
use options::Opt;
use url::Url;

use crate::gallery::GalleryItem;

pub type Result<T, E = error::Error> = std::result::Result<T, E>;

fn main() {
    if let Err(e) = run(&Opt::from_args()) {
        eprintln!("{}", e);
    }
}

fn run(opt: &Opt) -> crate::Result<()> {
    use gallery::*;

    let parsed_url = Url::parse(opt.url())?;
    let domain = parsed_url
        .domain()
        .ok_or_else(|| Error::Unsupported(UnsupportedError::Route, opt.url().into()))?;

    match domain {
        // "beta.sankakucomplex.com" => download(opt, sankakubeta::extract),
        // "e-hentai.org" => download(opt, ehentai::extract),
        // "fitnakedgirls.com" => download(opt, fitnakedgirls::extract),
        // "gelbooru.com" => download(opt, gelbooru::extract),
        // "imgur.com" => download(opt, imgur::extract),
        // "nhentai.net" => download(opt, nhentai::extract),
        // "nsfwalbum.com" => download(opt, nsfwalbum::extract),
        // "rule34.xxx" => download(opt, rule34::extract),
        // "www.f-list.net" => download(opt, flist::extract),
        // "www.girlswithmuscle.com" => download(opt, girlswithmuscle::extract),
        // "www.hentai-foundry.com" => download(opt, hentai_foundry::extract),
        "thefitgirlz.com" => download(opt, thefitgirlz::extract),

        other => Err(Error::Unsupported(UnsupportedError::Domain, other.into())),
    }
}

fn download<T: Gallery>(
    opt: &Opt,
    extractor: impl Fn(&str) -> crate::Result<T>,
) -> crate::Result<()> {
    let start_time = chrono::Local::now();

    let mut gallery = extractor(opt.url())?;
    if let Some(skip) = opt.skip {
        gallery.advance_by(skip)?;
    }

    let current_dir = env::current_dir()?;
    let overwrite = opt.overwrite();
    let waiter = opt
        .wait()
        .map(waiter::Waiter::from_secs)
        .unwrap_or_default();

    let mut storage = opt.storage_provider(&current_dir)?;
    let existing_files = read_existing_files(storage.path())?;

    let mut count = 0;
    let mut bytes_written = 0;
    let idx_offset = opt.skip.unwrap_or_default();

    while let Some(item) = gallery.next() {
        let idx = count + idx_offset;
        waiter.wait();

        match item {
            Ok(mut item) => {
                let path = storage.create_path(item.context());
                if !overwrite && existing_files.contains(&path) {
                    if let Some(file_path) = pathdiff::diff_paths(&path, &current_dir) {
                        println!(
                            "{} {} has already been downloaded",
                            idx + 1,
                            file_path.display()
                        );
                    }
                } else {
                    let mut target = File::create(&path)?;
                    bytes_written += item.write(&mut target)?;
                    if let Some(file_path) = pathdiff::diff_paths(&path, &current_dir) {
                        println!("{} {}", idx + 1, file_path.display());
                    }
                }
            }

            Err(e) => eprintln!("{} Warning: {}", idx + 1, e),
        }

        count += 1;
        if is_complete(count, opt.take) {
            break;
        }
    }

    let elapsed = chrono::Local::now().signed_duration_since(start_time);
    println!(
        "\n{} files ({})\nElapsed time {}",
        count,
        bytes_written.fmt_size(Conventional),
        elapsed.into_formatter(),
    );

    Ok(())
}

fn read_existing_files(path: impl AsRef<Path>) -> Result<HashSet<PathBuf>> {
    Ok(fs::read_dir(path)?
        .filter_map(|x| Some(x.ok()?.path()))
        .collect())
}

fn is_complete(count: usize, take: Option<usize>) -> bool {
    take.map(|take| take == count).unwrap_or_default()
}
