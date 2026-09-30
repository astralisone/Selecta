# Track2Mix marketing site

Static HTML/CSS/JS. No build step, no dependencies, no framework. Deploy the
`site/` directory as-is.

```
site/
  index.html     landing page
  styles.css     tokens mirrored from ../tailwind.config.ts
  main.js        email capture (needs one line of config, below)
  _headers       security + cache headers (Cloudflare Pages / Netlify)
  robots.txt
  assets/logo.svg
```

## Local preview

```bash
python3 -m http.server 8000 --directory site
# → http://localhost:8000
```

## Deploy free on Cloudflare Pages

1. Push this repo to GitHub.
2. Cloudflare dashboard → **Workers & Pages → Create → Pages → Connect to Git**.
3. Pick the repo. **Framework preset:** None. **Build command:** leave empty.
   **Build output directory:** `site`.
4. Deploy. You get `https://<project>.pages.dev` on free TLS, with a rebuild on
   every push to the branch you selected.

Netlify, Vercel and GitHub Pages all work the same way — the only setting that
matters is that the publish directory is `site`.

## Wire up email capture (required before launch)

`main.js` ships with `SIGNUP_ENDPOINT = ""`. Until you set it, the form tells the
visitor that signup is unconfigured and logs the reason to the console — it does
not fake a success. Set it to one of these:

| Provider | Free tier | Endpoint to paste |
| --- | --- | --- |
| Buttondown | 100 subscribers | `https://buttondown.com/api/emails/embed-subscribe/<your-username>` |
| MailerLite | 1,000 subscribers | Embedded-form action URL from the form's HTML snippet |
| Formspree | 50 submissions/month | `https://formspree.io/f/<your-form-id>` |

Buttondown and Formspree both expect the field name `email`, which is
`EMAIL_FIELD`'s default. MailerLite expects `fields[email]` — change
`EMAIL_FIELD` to match if you use it.

## Wire up the tip jar (required before launch)

`main.js` ships with `TIP_BASE_URL = ""`. Until you set it, the three tier
buttons render disabled with a visible note and the console says why — they
never link nowhere. The download stays free and working regardless.

| Provider | Platform fee | Notes |
| --- | --- | --- |
| **Ko-fi** | 0% on one-off tips | Recommended. Payment-processor fees only, no monthly cost, supports a suggested amount via `?amount=`. |
| Buy Me a Coffee | 5% | Slicker page, but you pay for it out of every tip. |
| GitHub Sponsors | 0% | Great fit for an open-source repo, but needs approval and skews developer rather than DJ. |
| Stripe Payment Link | 2.9% + 30¢ | Most professional and unbranded; needs a Stripe account and business details. |

`tipUrlFor()` appends `?amount=<n>`. Ko-fi, Buy Me a Coffee and Stripe
customer-chosen-amount links all accept that; if you switch to a provider that
names the parameter differently, that one function is the only thing to change.

### Where the tip ask actually converts

Not on the landing page. Put it where the value just landed:

1. **After a successful analysis run, inside the app.** The moment a DJ sees
   their library scored is the moment the tool has proven itself. This is by far
   the highest-converting placement and it costs nothing.
2. **On the post-download thank-you page.** Second best.
3. **In the release notes of each new version.** People who update are people
   who use it.

The landing-page tier block exists to set the expectation that paying is normal.
It is not where the money comes from.

## Before you launch

- [ ] Set `SIGNUP_ENDPOINT` in `main.js` and submit the form once to confirm the
      address actually lands in your list.
- [ ] Replace the hero preview with **real screenshots** of the app running on
      your own library. The markup in `index.html` under
      `<!-- Accurate rendering of the Track2Mix track view -->` is an honest
      rendering of the real columns with anonymised rows, but a genuine
      screenshot converts better and removes any doubt. A side-by-side of
      Rekordbox's Related Tracks against Track2Mix's picks for the same anchor
      track is the single most persuasive image you can put here.
- [ ] Add `assets/og.png` at 1200×630 for link previews — `index.html` already
      references it. Without it, shared links render without an image.
- [ ] Point the download button at a real release. Right now `#get` collects
      emails; once GitHub Releases has a signed build, link it directly.
- [ ] Update the GitHub URLs if the repo moves — they appear in `index.html`
      in the nav-adjacent CTA (`#gh-link`) and the footer.
- [ ] Turn on **Cloudflare Web Analytics** (free, no cookie banner needed) so
      you can see which channel actually sends traffic.
