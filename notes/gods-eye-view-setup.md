# God's Eye View — install & configure

Operator note, not part of the Track2Mix product. [God's Eye View][gev] (GEV) is a
local 3D-globe OSINT console — live flights, ships, satellites, quakes, fires, public
cameras. It runs entirely on your machine and starts with **no API keys**.

Every step below was executed end to end on a clean checkout; the results are at the
bottom. Verified **2026-09-30** against GEV `v0.1.1` (commit `e7707d9`) on Linux x64.

## 1. Node

GEV pins `node: >=24.14.0 <25 || >=26 <27`. Node 22 or 25 will not install — 25 is EOL
and the setup doctor rejects it.

```bash
nvm install 24 && nvm use 24     # or: brew install node@24 / volta install node@24
node -v                          # v24.21.0 — what this note was verified on
```

No version manager? Unpack the official tarball and put it first on `PATH`:

```bash
curl -fsSL https://nodejs.org/dist/v24.21.0/node-v24.21.0-linux-x64.tar.xz | tar -xJ
export PATH="$PWD/node-v24.21.0-linux-x64/bin:$PATH"
```

## 2. Install

```bash
git clone https://github.com/bilawalsidhu/gods-eye-view.git
cd gods-eye-view
npm ci
npm run doctor
npm run dev        # http://localhost:4173
```

Prefer no terminal at all? [Pinokio][pinokio] **8.2 or later** installs and launches it
with two clicks. Older Pinokio has a broken installer, and its native *Configure* panel
mishandles GEV's nested env file — use GEV's own POWER UP panel for keys either way.

Already have an old clone? Pull it. Versions before the Overpass fix are refused by the
public OpenStreetMap servers, so Traffic, Mapped Installations and ALPR stay empty.

### npm ≥ 11.19 defers install scripts

`npm ci` prints a warning that `esbuild` and `puppeteer` postinstalls were skipped. This
is fine for normal use — `npm run dev`, `npm run build` and `npm test` all pass without
them. Only the browser-driven QA scripts (`npm run qa:*`) need Chromium:

```bash
npm install-scripts approve puppeteer && npm install
```

## 3. Configure

**Keys are upgrades, not prerequisites.** Keyless, you get Esri World Imagery, keyless
terrain, OpenSky flights (anonymous, rate-limited), satellites, earthquakes, weather,
public cameras, radio and launches.

The intended way to add a key is in-app: the **POWER UP** chip, bottom-right →
*Provider Settings* → paste → **SAVE KEYS**. It writes the repo-root `.env` (creating it
owner-only *before* writing the secret) and restarts the dev server. If a narrow window
hides the chip, `?setup=1` reopens the panel. Keys already exported in your shell — or,
on macOS, held in the Keychain and pulled in by `./scripts/dev-fresh.sh` — show as
*configured externally* and are never overwritten.

For headless or self-hosted setups, edit `.env` directly:

```bash
cp .env.example .env && chmod 600 .env
```

`notes/gods-eye-view.env.starter` in this repo is a trimmed version of that file — the
variables most people actually set, without the ~150 lines of CCTV source tuning.

| Key | Unlocks | Cost | Exposure |
|---|---|---|---|
| `CESIUM_ION_TOKEN` | Google Photorealistic 3D via ion, world terrain | free tier, personal/non-commercial | **client-exposed** — use an `assets:read` token with URL restrictions |
| `GOOGLE_MAPS_API_KEY` | Direct Google 3D Tiles + place search | metered | **client-exposed** — restrict by HTTP referrer + API |
| `GOOGLE_MAPS_SERVER_API_KEY` | Places / Street View server calls | metered | server-only; restrict by IP. Falls back to the key above if unset |
| `OPENAI_API_KEY` | Realtime voice control | metered | server-only |
| `AISSTREAM_API_KEY` | Live worldwide vessels | free | server-only |
| `FIRMS_MAP_KEY` | NASA live active-fire detections | free | server-only |
| `TOMTOM_API_KEY` | Real traffic flow (else built-in simulation) | free tier, 200k tiles/month | server-only |
| `OPENSKY_CLIENT_ID` / `_SECRET` | Higher flight-data rate limits | free | server-only |
| `LL2_API_TOKEN` | Higher Launch Library 2 allowance | free | server-only |

Start with the Cesium ion token — it is the one that turns the globe photorealistic, and
it is free for eligible personal use. Add a Google key only if you want the direct,
billed tile route or Google place search.

The two client-exposed keys are **visible in devtools by design**. Scope them at the
provider rather than trying to hide them.

### Keep it local

`HOST` defaults to localhost, and it should stay there. The dev server brokers your keys
on behalf of the browser, so anything that can reach it can spend your quota. If you do
set `HOST=0.0.0.0` for LAN access, also set the per-IP throttles —
`GEV_RATELIMIT_GOOGLE_PER_MIN` and `GEV_RATELIMIT_OPENAI_PER_MIN`, both unlimited by
default. They are in-memory guards, not billing caps: set real budget alerts in Google
Cloud Console and OpenAI → Settings → Limits.

## Verified results

| Step | Result |
|---|---|
| `npm ci` | 131 packages, 0 vulnerabilities, 27s |
| `npm run doctor` | all green — Node, npm, dependencies; every provider `[--]` (keyless) |
| `npm run dev` | Vite 6.4.3 ready in 1.3s, `GET /` → 200 |
| `GET /api/setup/status` | 200, full provider inventory — server proxies are live |
| `npm run build` | built in 20s (chunk-size warnings only) |
| `npm test` | 13/13 pass, 109s |

Keyless upstream feeds answer to their own rate limits — an anonymous OpenSky call
returning 503 is throttling, not a broken install. Retry, or add OpenSky credentials.

[gev]: https://github.com/bilawalsidhu/gods-eye-view
[pinokio]: https://pinokio.co/apps/github-com-bilawalsidhu-gods-eye-view
