use std::{
    collections::HashSet,
    env,
    error::Error,
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

use fmtsize::{Conventional, FmtSize};
use gallery::{DynamicGallery, Gallery, GalleryItem};
use options::Opt;

pub type Result<T, E = error::Error> = std::result::Result<T, E>;

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let gallery = identify(&opt)?;
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

    for (idx, item) in apply_paging(&opt, gallery)?.enumerate() {
        let idx = idx + idx_offset;
        waiter.wait();

        // We don't necessarily want to bail on just any item error.
        match item {
            Err(e) => match e.source() {
                Some(source) => eprintln!("{} Warning: {}\n  {}", idx + 1, e, source),
                None => eprintln!("{} Warning: {}", idx + 1, e),
            },

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

fn identify(opt: &Opt) -> Result<DynamicGallery> {
    use config::{Configuration, Key};
    use error::{Error, UnsupportedError};
    use gallery::{
        EHentai, FList, FitNakedGirls, Gelbooru, GirlsWithMuscle, HentaiFoundry, ImgurAlbum,
        ImgurGallery, ImgurSingle, NHentai, NsfwAlbum, ReadGallery, Rule34, Sankaku, SankakuBeta,
    };
    use url::Url;

    let address = Url::parse(opt.url())?;
    match address.domain() {
        Some("beta.sankakucomplex.com") => SankakuBeta.read(opt.url()),
        Some("e-hentai.org") => {
            let config = Configuration::init();
            let username = config.get_config(Key::EHentaiUser)?;
            let password = config.get_config(Key::EHentaiPass)?;
            EHentai::new(username, password).read(opt.url())
        }

        Some("fitnakedgirls.com") => FitNakedGirls.read(opt.url()),

        Some("gelbooru.com") => {
            let config = Configuration::init();
            let user_id = config.get_config(Key::GelbooruUser)?;
            Gelbooru::new(user_id).read(opt.url())
        }

        Some("www.hentai-foundry.com") => HentaiFoundry.read(opt.url()),
        Some("www.girlswithmuscle.com") => GirlsWithMuscle.read(opt.url()),

        // Imgur presents three different types of galleries, which are implemented separately
        Some("imgur.com") if address.path().starts_with("/a/") => ImgurAlbum.read(opt.url()),
        Some("imgur.com") if address.path().starts_with("/gallery/") => {
            ImgurGallery.read(opt.url())
        }
        Some("imgur.com") => ImgurSingle.read(opt.url()),

        Some("nhentai.net") => NHentai.read(opt.url()),
        Some("nsfwalbum.com") => NsfwAlbum.read(opt.url()),
        Some("rule34.xxx") => Rule34::new().read(opt.url()),
        Some("www.f-list.net") => FList.read(opt.url()),
        Some("chan.sankakucomplex.com") => Sankaku.read(opt.url()),

        // Our first error case is an unsupported domain, but the second is just an invalid url.
        Some(domain) => Err(Error::Unsupported(UnsupportedError::Domain, domain.into())),
        None => Err(Error::Unsupported(
            UnsupportedError::Route,
            opt.url().into(),
        )),
    }
}

fn apply_paging(
    opt: &Opt,
    mut gallery: Box<dyn Gallery>,
) -> Result<Box<dyn Iterator<Item = Result<GalleryItem>>>> {
    if let Some(skip) = opt.skip {
        gallery.apply_skip(skip)?;
    }

    match opt.take {
        Some(take) => Ok(Box::new(gallery.take(take))),
        None => Ok(Box::new(gallery)),
    }
}
