# Sankaku Complex

## Search pages

First thing first, Sankaku spews a 500 error when you attempt to curl the damn site. Spoofing a useragent string with curl is trivial: pass it as the parameter for the -A flag.

```shell
curl 'https://chan.sankakucomplex.com/?tags=abs+sweat&commit=Search' -A 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36' > resource/sankaku/search-results.html
```

...Honestly, what was the point of that? Who's gonna A) curl your site who doesn't know how to B) Google "curl user agent string"? Hope it was fun configuring nginx to do that, because you didn't achieve anything else. Except, you know, annoying me.

The site works basically the following way: initial requests send back the entire page (search-results.html) and subsequent page requests send back just one page, which I think is loaded asynchonously as you scroll down, and I also think it tries to stay a little bit ahead of what you're actually looking at, but I don't see that second part as mattering much.

This is an example of the network call made when grabbing extra shiz:

```
https://chan.sankakucomplex.com/post/index.content?next=13314641&tags=abs+sweat&page=3
```

(Yeah. Abs and sweat. I know what I like. Fuck off.)

This call, made in curl, returns a set of spans containing the html required to display image thumbnails and so forth. This works apparently even when the original call hasn't been made yet, sooooo... You know, happy days.

All right, it looks like the call to the paging mechanism must include the first id of the next image set (??!) as well as the page number. That's not a great big deal, but I imagine it means we will need to make the first call the old fashioned way, unless we can skip the next param in the initial call. Lemme try that out...

YES. Paging mechanism pwned! (experimental-results.html) If you skip the next and page parameters, you can just make that call and it spits back something short and sweet and at least marginally useful. What we get, specifically, is this tag:

```
<div next-page-url="/?next=13698738&amp;tags=abs%20sweat&amp;page=2">
```

...which we can use to know which url to call next without having to do any hoofing, and all of these identifiers:

```
/post/show/13868544
/post/show/13865073
/post/show/13860174
/post/show/13859286
/post/show/13858092
/post/show/13812415
/post/show/13812388
/post/show/13812349
/post/show/13812313
/post/show/13812223
/post/show/13812191
/post/show/13812157
/post/show/13812033
/post/show/13811955
/post/show/13811375
/post/show/13782866
/post/show/13722561
/post/show/13711594
/post/show/13701080
/post/show/13699140
```

...which represent a full page of twenty images. All we really need from here, after I write the expressions required to extract this information, is a mechanism to extract the image url itself from those pages.

Next page url extractor:
```
next-page-url="([^"]+)"
```

Image page url extractor:
```
a href="/post/show/(\d+)"
```

## Post pages

Here's our test page: `https://chan.sankakucomplex.com/post/show/6552810`

![Yeah, it's Korra. Shut up.](kas.png "Yeah, it's Korra. Shut up.")

That page represents kind of a best case/worst case scenario, depending on how many pixels you like to receive: the image is only 800x1088, which means there's no larger version hiding somewhere. The following test page is an example of one where the image is shrunk down initially instead:

`https://chan.sankakucomplex.com/post/show/4703624` (post-page-with-preview.html)

The full image case:

```html
<a id="image-link" class="full">
<img alt="avatar: the last airbender avatar: the legend of korra avatar (series) korra for a 1girl abs areolae armpits big lips blush breasts brown hair close-up dark skin dark-skinned female detached sleeves female female only large breasts long hair muscle muscular female nipples puffy areolae serratus anterior shiny shiny skin sideboob side view solo sweat sweatdrop toned" id="image" onclick="Note.toggle();" orig_height="1088" orig_width="800" src="//cs.sankakucomplex.com/data/2d/ad/2dadb16fc569d7394af164618e398208.png?e=1562117913&amp;m=UZ2Sp0c663qGz8rk0Bphhg" pagespeed_url_hash="2022131225" onload="pagespeed.CriticalImages.checkImageForCriticality(this);" width="800" height="1088">
</a>
```

The preview image case:

```html
<a id="image-link" class="sample" href="//cs.sankakucomplex.com/data/67/2b/672badc6ddfd43cb695a03136101ac5e.png?e=1562118395&amp;m=hC48mUmjTJRN-lp8KVZWNQ">
<img alt="avatar: the last airbender avatar: the legend of korra nickelodeon cutepet korra naga (avatar) high resolution large filesize 3:2 aspect ratio 1girl abs ass barefoot blush breasts brown hair canine clitoris dark skin dog eyes closed feet female female focus large breasts muscle muscular female navel nipples nude ponytail pubic hair semen semen on body semen on breasts semen on upper body sleeping solo focus spread legs sweat thighs tied hair vagina vaginal juices" id="image" onclick="Note.toggle();" orig_height="2025" orig_width="3030" src="//cs.sankakucomplex.com/data/sample/67/2b/sample-672badc6ddfd43cb695a03136101ac5e.jpg?e=1562118395&amp;m=cwXfrlLoUG_Dot3D0e6LjQ" pagespeed_url_hash="810940079" onload="pagespeed.CriticalImages.checkImageForCriticality(this);" width="1400" height="935">
</a>
```

In either case, we have an `<a id="image-link">`, but only in the latter case does the image link actually *link* to something. In the former case, a "link" is provided that goes nowhere, and the user... What? What the hell was the user supposed to do with a link to nowhere? It's a lazy fucking way to do business, I say! `</fakerant>`

So, effectively, we use two image link extractors. One will pull the full size link and the other will pull the standard link as a fallback. Use the latter if the former doesn't hit on anything.

Primary extractor:
```
<a id="image-link" class="sample" href="([^"]+)">
```

Secondary extractor:
```
<img[^>]+src="([^"]+)"[^>]+>
```

I know that second one looks like one of those table-flip emojis or something, but roll with it.

## THE UNKNOWN!!1!1!

Sankaku Complex boasts a pretty strict "u dawnlod too fass round3x3y3!" policy, and I'm not sure what to do about that. This program is almost certainly going to run afoul of that policy. It's possible we can just handle the 429 error when it comes, but I'd rather not incur the wrath of SANKAKUCOMPREX in the first place, if possible.

Entertainingly, if you enter "too many requests" into my browser's search bar, the suggested result is not a search page about error 429 but instead a previously visited page on Sankaku. `>.<`

## Fun test search

This might make for a fun test search:
```
https://chan.sankakucomplex.com/?tags=korra+whip_marks&commit=Search
```

## Testing update

I found that the extraction strategy I had chosen was mostly wrong. For one thing, the regular expression I wrote initially assumed that they would put their ids and classes in quotes. They do not. I have no idea how I missed that. Here are the new patterns. I'm grabbing the fallback size from the content meta tag, because the actual image tag containing the sample image is just too generic to target. I kept getting the damned logo.

```rust
Ok(Box::new(SankakuGallery {
    client,
    next_url,
    queue,
    next_page_url_pattern,
    image_post_url_pattern,
    full_image_pattern: Regex::new(r#"<a id=image-link class=sample href="([^"]+)">"#)
        .unwrap(),
    sample_image_pattern: Regex::new(r#"<meta content="([^"]+)" property=og:image>"#).unwrap(),
}))
```
