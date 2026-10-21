# Google Search Console (Phase 1)

Property: **https://vanity-address.pages.dev/**

Do this once after the SEO landing deploy is live on `main` (Cloudflare Pages).

## 1. Add property

1. Open [Google Search Console](https://search.google.com/search-console)
2. **Add property** → URL prefix → `https://vanity-address.pages.dev`
3. Prefer **HTML tag** verification (works on Pages without DNS ownership of `pages.dev`)

## 2. HTML tag verify

1. Copy the `content="…"` token from Google
2. In `landing/index.html`, find the Phase 1 GSC comment near `<link rel="canonical"…>`
3. Uncomment and set:

```html
<meta name="google-site-verification" content="YOUR_TOKEN" />
```

4. Commit + push `main` → wait for Pages deploy
5. Click **Verify** in Search Console

Alternative: DNS TXT on a future **custom domain** (recommended later for trust).

## 3. Submit sitemap

1. Search Console → **Sitemaps**
2. Submit: `https://vanity-address.pages.dev/sitemap.xml`
3. Expect URLs: home, `/generate/`, Solana / Ethereum / Bitcoin / CREATE2 guides, how-to, `llms.txt`

## 4. Request indexing (first wave)

URL Inspection → **Request indexing** for:

- `https://vanity-address.pages.dev/`
- `https://vanity-address.pages.dev/generate/`
- `https://vanity-address.pages.dev/solana-vanity-address/`
- `https://vanity-address.pages.dev/ethereum-vanity-address/`
- `https://vanity-address.pages.dev/bitcoin-vanity-address/`
- `https://vanity-address.pages.dev/create2-vanity-address/`

## 5. After 2–4 weeks

- **Performance** → Queries: expand copy for terms that already get impressions
- Fix **Coverage** / **Page indexing** errors if any
- Do **not** spam re-requests; re-index after real content changes

## Related

- Long-tail guides live under `landing/*-vanity-address/`
- Sitemap: `landing/sitemap.xml`
- robots: `landing/robots.txt` already points at the sitemap
