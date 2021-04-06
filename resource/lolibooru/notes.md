# Lolibooru

I'm guessing this is similar to all the other booru sites and that I can reuse some techniques, but I've already noted that there is no anonymous account functionality here. That said, it looks to me like it won't matter much because as far as I can tell the images are still coming through over the network and are just being hidden via a style. We'll see.

Sample page:

```
https://lolibooru.moe/post?tags=sabine_heinrich&page=2
```

Ok, in fact, it looks as if each thumbnail is accompanied by a direct link to the full resolution image? It looks like some of them say large image and some say small image, but that appears to refer to the site's categorization of the image rather than to the idea that there is more than one version.

```
<a class="[^"]*directlink[^"]*" href="([^"]+)">
```

As for paging, it looks like we can just increment the page parameter on any given search until we run out of results.
