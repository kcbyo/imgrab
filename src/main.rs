mod config;
mod error;
mod format;
mod gallery;
mod options;
mod storage;
mod tags;
mod waiter;

use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    path::{Path, PathBuf},
};

use error::{Error, UnsupportedError};
use fmtsize::{Conventional, FmtSize};
use format::DurationFormat;
use gallery::Gallery;
use options::Opt;
use url::Url;

use crate::gallery::GalleryItem;

pub type Result<T, E = error::Error> = std::result::Result<T, E>;

fn main() {
    if let Err(e) = run(&Opt::parse()) {
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
        "beta.sankakucomplex.com" => download(opt, sankakubeta::extract),
        "e-hentai.org" => download(opt, ehentai::extract),
        "fitnakedgirls.com" => download(opt, fitnakedgirls::extract),
        "gelbooru.com" => download(opt, gelbooru::extract),
        "hdporn.pics" => download(opt, hdporn::extract),
        "imgur.com" => download(opt, imgur::extract),
        "nhentai.net" => download(opt, nhentai::extract),
        "nsfwalbum.com" => download(opt, nsfwalbum::extract),
        "rule34.us" => download(opt, rule34_us::extract),
        "rule34.xxx" => download(opt, rule34::extract),
        "thefitgirlz.com" => download(opt, thefitgirlz::extract),
        "www.beautymuscle.net" => download(opt, beautymuscle::extract),
        "www.f-list.net" => download(opt, flist::extract),
        "www.girlswithmuscle.com" => download(opt, girlswithmuscle::extract),
        "www.hentai-foundry.com" => download(opt, hentai_foundry::extract),

        other => Err(Error::Unsupported(UnsupportedError::Domain, other.into())),
    }
}

fn download<T: Gallery>(
    opt: &Opt,
    extractor: impl Fn(&str) -> crate::Result<(T, Option<String>)>,
) -> crate::Result<()> {
    let start_time = chrono::Local::now();

    let (mut gallery, gallery_name) = extractor(opt.url())?;

    if let Some(skip) = opt.skip {
        gallery.advance_by(skip)?;
    }

    let current_dir = env::current_dir()?;
    let canonical_base_dir = current_dir.canonicalize()?;
    let overwrite = opt.overwrite();
    let waiter = opt
        .wait()
        .map(waiter::Waiter::from_option)
        .unwrap_or_default();

    let mut storage =
        opt.storage_provider(&current_dir, gallery_name.as_ref().map(AsRef::as_ref))?;
    let existing_files = read_existing_files(storage.path())?;

    let mut count = 0;
    let mut bytes_written = 0;
    let idx_offset = opt.skip.unwrap_or_default();

    while let Some(item) = gallery.next() {
        let idx = count + idx_offset;
        waiter.wait();

        match item {
            Ok(item) => {
                let path = storage.create_path(item.context());
                if !overwrite && existing_files.contains(&path) {
                    if let Ok(file_path) = shorten_path(&canonical_base_dir, &path) {
                        // We have just found an existing file. If we've been asked to stop
                        // downloading after finding an existing file, we won't bother printing
                        // the name of the file.

                        if opt.take_new {
                            break;
                        }

                        println!(
                            "{} {} has already been downloaded",
                            idx + 1,
                            file_path.display()
                        );
                    }
                } else {
                    let mut target = File::create(&path)?;
                    bytes_written += item.write(&mut target)?;
                    if let Ok(file_path) = shorten_path(&canonical_base_dir, &path) {
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
        "\n{} files ({})\n{} elapsed",
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

fn shorten_path(canonical_base_dir: &Path, path: &Path) -> Result<PathBuf> {
    Ok(path
        .canonicalize()?
        .strip_prefix(canonical_base_dir)
        .unwrap_or(path)
        .into())
}
