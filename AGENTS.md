# AGENTS.md

Instructions for AI coding agents (Codex, Claude Code, others) working in this repo.

## Talk to the user in Thai

The user (project owner; maintains frontend and backend alone since 2026-11-24) prefers Thai in conversation. Code, comments, commit messages and docs are in English.
User-facing copy on the website stays in Thai.

## What this repo is

Monorepo for MaeMorKratai (แม่หมอกระต่าย), a Thai tarot website. Remote: `git@github.com:riwjung/MaeMorKrataiCenter.git`, branch `main`.
Local checkout: `~/AIVideoLocal/mmk-platform/` (kept off the iCloud Desktop on purpose; iCloud evicts files and breaks builds).

All folders are maintained by the user. The backend scaffold was built by a friend who has stopped working on it; its specs, decisions and plan records in `backend/docs/` explain why it is built the way it is.

| Folder | State |
|---|---|
| `frontend-client/` | Customer tarot site. Next.js 16 static export, live on Cloudflare Workers, no backend calls yet. |
| `frontend-admin/` | Empty. Future dashboard, reader scoring page, content automation control. |
| `backend/` | Rust Workers `client/` and `admin/` sharing D1/R2. Admin auth (SPEC-001) is done and tested; SPEC-002 (master data) is approved, not built. Nothing deployed yet. |
| `docs/` | Project context (English), API contracts, reading rules. |

## Read before working

1. `README.md` for the rules
2. `docs/context/01-overview.md` to `08-open-decisions.md`, plus `docs/context/glossary.md`
3. Frontend work: `frontend-client/README.md` and `frontend-client/AGENTS.md` (Next.js 16 note: read `node_modules/next/dist/docs/` before using Next APIs)
4. Backend work: `backend/README.md`, `backend/HANDOFF.md`, `backend/TODO.md`, then `backend/CLAUDE.md` (gb-sdlc rules)

## Rules

- Frontend and backend talk only through contracts in `docs/api/`. Never import code across folders.
- Never commit secrets. `.env*` is ignored; commit `.env.example` with placeholders.
- Never add real reader answers, audio, or holdout data. Never invent tarot meanings; content comes from the reader.
- Ask before pushing, deploying, or spending money on any paid service.
- Commit and push only when the user asks.

## frontend-client commands

```sh
cd frontend-client
npm ci
npm run dev        # http://127.0.0.1:3010/
npm run typecheck
npm run build      # static export to out/
```

## backend commands

```sh
cd backend
make help          # every target, with [CLOUD] on the ones that change Cloudflare
make test          # cargo test --workspace
make check         # wasm32 type-check of both Workers
```

`[CLOUD]` targets (`cf-bootstrap`, `cf-secret`, `migrate-staging|prod`, `deploy-*`) change real Cloudflare state: ask first.
Disk note: the Mac has little free space; a Rust build needs a few GB in `backend/target/`.

## frontend-client deployment (done)

Decision: Cloudflare **Workers with Static Assets** (not Pages). The user has never used Cloudflare; explain each step.

Preparation is implemented: Wrangler is installed, `wrangler.jsonc` is present,
and `cloudflare/worker.mjs` handles MP4 byte ranges for Safari. Static Assets alone
returns full-file responses to range requests. The Worker runs first only for
`/scene/*.mp4`; its requests count toward the Workers quota. Run
`npm run test:cloudflare` to check range handling.

First-time instructions: `frontend-client/docs/cloudflare-deployment.md`.
The first CLI deployment succeeded on 2026-11-16 at
`https://mmk-frontend-client.maemorkratai.workers.dev` (version
`cfd8a11d-74ae-4e1f-9219-ec638f46cca9`). GitHub is connected for production
builds from `main`. The first Git-triggered build still needs verification;
the build root must be `frontend-client`, not `/`.

Plan:
1. Review `frontend-client/wrangler.jsonc`; use the checked-in configuration, including the video Worker.
2. `npm run preview:cloudflare` and check `/` and `/reading/` at http://127.0.0.1:8787
3. User runs `npx wrangler login` and `npm run deploy:cloudflare` (needs their Cloudflare account) → `*.workers.dev` URL
4. Dashboard → Worker → Settings → Build → connect GitHub repo. Root directory `frontend-client`, build `npm run build`, deploy `npx wrangler deploy`, build watch path `frontend-client/*`, non-main branches get preview URLs
5. Custom domain via Settings → Domains & Routes (if the user has a domain)
6. Keep the current Vercel site (https://mae-mor-kratai.vercel.app/) live until Cloudflare is verified on mobile, Safari, the shuffle video and `/reading/`. `frontend-client/vercel.json` can be removed after the switch.

Later: `frontend-admin` as a second Worker behind Cloudflare Access (only the owner and the reader).

## Open items

- Agent instructions and the daily-reading entry update are committed. `next-env.d.ts` returned to the tracked build paths after production build.
- Daily-reading UI commit `0592d73` was deployed to the existing Vercel production URL on 2026-11-16 (Asia/Bangkok), deployment `dpl_GLpbau9tURX4Q2HrweJaiR1Y9bW6`.
- Old project with history and legacy apps: `/Users/sawettachatsuthum/Desktop/AI_Video/MaeMorKratai` (repo `riwjung/MaeMorKratai`). Reference only; do not modify.
