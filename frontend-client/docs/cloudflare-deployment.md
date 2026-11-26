# Cloudflare Workers with Static Assets

The customer site is a Next.js static export. `npm run build` writes `out/`;
Wrangler uploads that directory. No Next.js server or OpenNext adapter is needed.

The Worker name is `mmk-frontend-client`. The configuration is in `../wrangler.jsonc`.
`../cloudflare/worker.mjs` adds HTTP byte ranges for scene MP4s so Safari can play
the idle and shuffle videos. Other assets are served directly by Static Assets.
Video requests invoke Worker code and count toward the account's Workers quota.

## Deployment status (2026-11-16)

The first CLI deployment succeeded at
<https://mmk-frontend-client.maemorkratai.workers.dev>, version
`cfd8a11d-74ae-4e1f-9219-ec638f46cca9`. GitHub is now connected to the Worker
with production branch `main`. Automatic deployment is awaiting its first
verified build. Set the dashboard build root to `frontend-client`; `/` has no
`package.json` and cannot run `npm run build` for this monorepo.

## 1. Create an account

1. Open <https://dash.cloudflare.com/sign-up>.
2. Create an account, verify the email address, and sign in.
3. Start with the Workers Free plan. A custom domain is optional; the first deployment
   receives a `workers.dev` address. Do not upgrade a paid plan without owner approval.

## 2. Preview locally

Open Terminal and run:

```sh
cd ~/AIVideoLocal/mmk-platform/frontend-client
npm ci
npm run test:cloudflare
npm run preview:cloudflare
```

Open <http://127.0.0.1:8787/> and <http://127.0.0.1:8787/reading/>.
Check the daily-reading heading and button, then draw a card. Press Ctrl+C in
Terminal to stop the preview before continuing.

## 3. Sign in and deploy

In the same directory:

```sh
npx wrangler login
```

The browser opens Cloudflare's permission page. Sign in and choose Allow.
Never paste account passwords or tokens into chat. Back in Terminal, check the account:

```sh
npx wrangler whoami
```

If more than one account appears, select the intended account when prompted during
deployment. Then run:

```sh
npm run deploy:cloudflare
```

This command rebuilds the frontend before uploading it. If prompted to register a
Workers subdomain, choose an available name. Copy the resulting address, for example
`https://mmk-frontend-client.<your-subdomain>.workers.dev`.

## 4. Check the deployed site

On the Cloudflare address, check:

- `/`: the updated daily-reading entry, drawing a card, reopening it, and reading details.
- `/reading/`: the existing three-card demo.
- iPhone/Safari: idle animation and shuffle video; the card reveal must finish.
- A nonexistent path: an HTTP 404 page rather than a duplicate home page.

Keep <https://mae-mor-kratai.vercel.app/> live until these checks pass.
The daily-reading UI from commit `0592d73` was deployed to that Vercel production
address on 2026-11-16 (Asia/Bangkok), deployment `dpl_GLpbau9tURX4Q2HrweJaiR1Y9bW6`.

## 5. Connect GitHub for automatic deployments

This step is optional for the first CLI deployment. The Cloudflare setup must be
pushed and merged through a pull request into `main` first. A local commit alone
is not visible to Cloudflare. Agents must get authorization before pushing.

1. Open Cloudflare Dashboard → Workers & Pages → `mmk-frontend-client`.
2. Open Settings → Build and connect the Git repository.
3. Authorize the Cloudflare GitHub app for `riwjung/MaeMorKrataiCenter`.
4. Set:

| Setting | Value |
|---|---|
| Worker name | `mmk-frontend-client` |
| Production branch | `main` |
| Root directory | `frontend-client` |
| Build command | `npm run build` |
| Deploy command | `npx wrangler deploy` |
| Build watch include path | `frontend-client/*` |

5. Enable non-production branch builds if preview URLs are wanted. Keep the
   dashboard's default preview command; previews must not deploy to production.
6. Save and run the first build. Open its build log and confirm success.

The Worker name in the dashboard and `wrangler.jsonc` must match. There are no
frontend environment variables to copy from Vercel for this static site.

## 6. Add a domain later

A domain is optional. When a domain is active in the same Cloudflare account,
open the Worker → Settings → Domains & Routes → Add → Custom Domain, enter the
hostname, and follow the DNS/certificate setup. Verify the domain before replacing
public links. Keep the Vercel fallback during the migration.

## References

- [Static Assets setup](https://developers.cloudflare.com/workers/static-assets/get-started/)
- [Static generation and 404 routing](https://developers.cloudflare.com/workers/static-assets/routing/static-site-generation/)
- [Static Assets limits, pricing and video range requests](https://developers.cloudflare.com/workers/static-assets/billing-and-limitations/)
- [Workers Builds configuration](https://developers.cloudflare.com/workers/ci-cd/builds/configuration/)
- [Build watch paths](https://developers.cloudflare.com/workers/ci-cd/builds/build-watch-paths/)
- [Custom Domains](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/)

## Master data from the API (2026-11-26)

The site reads its master data (cards, daily cards, card art sizes, reading taxonomy) from the backend
client API when its hostname is mapped in `src/lib/master-data.ts`:

| Site hostname | API |
| --- | --- |
| `mmk-frontend-client.maemorkratai.workers.dev` | `https://maemorkratai-backend-client-staging.maemorkratai.workers.dev` |
| `staging.maemorkratai.com` | same staging API |
| anything else (Vercel, production, local) | none: the bundled JSON in `src/lib/` |

- The bundled JSON renders the first paint. The API bodies replace it once all four requests succeed and
  pass validation (`src/lib/master-data-adapter.ts`). On any failure the page keeps the bundled data.
- `<html data-master-data="api">` or `"bundled"` shows which data a page renders.
- `NEXT_PUBLIC_MASTER_DATA_API` at build time overrides the map, for testing against a local
  `wrangler dev` of the backend client (its dev CORS allows `http://127.0.0.1:3000`).
- `npm run test:data` checks that the API bodies (`../backend/seed/*.json`) adapt to exactly the bundled
  data, and that broken bodies are rejected.
- Pushing to `main` builds and deploys this Worker automatically (observed 2026-11-26 after a push).
