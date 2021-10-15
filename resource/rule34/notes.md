# Notes

Search pages are not protected against bots. (Depends on IP?)

Image pages can be extracted from search pages using:

    index\.php\?page=post&s=view&id=(\d+)

Images can be filtered via their thumbnail filename, because the thumbnail filename includes the MD5 hash of the image:

    https://us.rule34.xxx/thumbnails/4564/thumbnail_d6085ef99a6b4c3e427de4f2a3e349f8.jpg?5198668

Doing this would permit us to avoid making some requests (but really isn't included in the imgrab application's capabilities at the moment).

Pretty sure we always (ALWAYS) get 42 files per request. That might or might not help with paging, because the paging might be based on a "last file" or something, which would require us to fetch page 3 before we can fetch page 4, etc.

## Image metadata

The images page provides an image metadata object in pseudojson that will allow us to generate a url of the following form:

    https://us.rule34.xxx/images/4558/ca038db132306d37018ecb0299e6d9e2.png

This url *appears to be* unprotected by rule34's crazy bot defense scheme. (Either that, or my IP address is no longer held in contempt? That could be because I'm not on NordVPN at the moment.)

Original pseudojson (actually JavaScript):

    image = { 'domain': 'https://us.rule34.xxx/', 'width': 3085, 'height': 4047, 'dir': 4558, 'img': 'ca038db132306d37018ecb0299e6d9e2.png', 'base_dir': 'images', 'sample_dir': 'samples', 'sample_width': '850', 'sample_height': '1115' };

Extractable via:

    image = (\{.+\})

We then need to transform single quotes into double quotes prior to parsing as json.

## Cookies

- gdpr=1
- gdpr-disable-ga=1
- resize-notification=1
- resize-original=1
