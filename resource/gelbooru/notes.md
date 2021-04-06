# Gelbooru

First thing to know about Gelbooru is that Gelbooru allows for some anonymous user account nonsense. For simplicity's sake, for both my part and theirs, I'm going to require that a user account be provided in the environment variables. If we do that, it may be possible to get around a lot of horseshit with them.

## API

I just found out that there is an API. There's even API documentation. Exploring...

The following url results in the file stored as [search.json](search.json):

```
https://gelbooru.com/index.php?page=dapi&s=post&q=index&limit=10&tags=loli+slave&json=1
```

```
https://gelbooru.com/index.php?page=dapi&s=post&q=index&limit=10&tags=loli+slave&json=1&pid=0
```

The json document will be perfect, of course. Only catch is that they may throttle the API, but I figure that's their right and I want to try to stay within their limits as much as I can.

The limit parameter there maxes out at 100. We should probably use the max. Paging is handled by another parameter called pid.
