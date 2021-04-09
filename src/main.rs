use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    path::{Path, PathBuf},
};

mod config;
mod error;
mod gallery;
mod options;
mod storage;
mod tags;
mod waiter;

use error::{Error, UnsupportedError};
use fmtsize::{Conventional, FmtSize};
use gallery::Gallery;
use options::Opt;
use url::Url;

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
        "beta.sankakucomplex.com" => download(opt, sankakubeta::extract(opt.url())?),
        "e-hentai.org" => download(opt, ehentai::extract(opt.url())?),
        "fitnakedgirls.com" => download(opt, fitnakedgirls::extract(opt.url())?),
        "gelbooru.com" => download(opt, gelbooru::extract(opt.url())?),
        "imgur.com" => download(opt, imgur::extract(opt.url())?),
        "nhentai.net" => download(opt, nhentai::extract(opt.url())?),
        "nsfwalbum.com" => download(opt, nsfwalbum::extract(opt.url())?),
        "rule34.xxx" => download(opt, rule34::extract(opt.url())?),
        "www.f-list.net" => download(opt, flist::extract(opt.url())?),
        "www.girlswithmuscle.com" => download(opt, girlswithmuscle::extract(opt.url())?),
        "www.hentai-foundry.com" => download(opt, hentai_foundry::extract(opt.url())?),

        other => Err(Error::Unsupported(UnsupportedError::Domain, other.into())),
    }
}

fn download(opt: &Opt, mut gallery: impl Gallery) -> crate::Result<()> {
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
    let existing_files = read_existing_files(&current_dir)?;

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
                    if let Ok(path) = path.strip_prefix(&current_dir) {
                        println!("{} {} has already been downloaded", idx + 1, path.display());
                    }
                    continue;
                }

                let target = File::create(&path)?;
                count += 1;
                bytes_written += item.write(target)?;
                if let Ok(path) = path.strip_prefix(&current_dir) {
                    println!("{} {}", idx + 1, path.display());
                }
            }

            Err(e) => eprintln!("{} Warning: {}", idx + 1, e),
        }

        if is_complete(count, opt.take) {
            break;
        }
    }

    println!(
        "\n{} files ({})",
        count,
        bytes_written.fmt_size(Conventional),
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
