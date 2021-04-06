# Filters

Site uses filters which appear to be set in session data on the server side. Or something. Filters must be sent with a CSRF token, I... Assume. Whatever. The token appears multiple times on the page, but its value is stable across multiple forms and, seemingly, across multiple loads. Not sure what the hell it's being used for yet.

Some of these filters are represented by a checkbox. Those are sent twice in the original packet from the website (once with a zero, once with a 1). Not sure whether we need to emulate the same behavior, but the sample form submission below has them removed.

I'm going to try to handle at least some of this nonsense (the PHPSESSID, for instance) using reqwest's `cookie_store` feature. We'll see how that goes.

```
YII_CSRF_TOKEN=WHBSWk11Z3FtbElfY2VNU1c3dW55ZEVyRXg0QVJnX0LHmW8uAUsxbEdHK6_wY5fDUJKhMinITx1RzEIEJXEXTQ%3D%3D
rating_nudity=3
rating_violence=3
rating_profanity=3
rating_racism=3
rating_sex=3
rating_spoilers=3
rating_yaoi=1
rating_yuri=1
rating_teen=1
rating_guro=1
rating_furry=1
rating_beast=1
rating_male=1
rating_female=1
rating_futa=1
rating_other=1
rating_scat=1
rating_incest=1
rating_rape=1
filter_media=A
filter_order=date_new
filter_type=0
```

## Note on user-gallery.txt

I've changed the extension to .txt because otherwise the file produces annoying linting errors.
