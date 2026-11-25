# 08 · Open decisions

When a decision is made, write it up in [`../decisions/`](../decisions) as `NNNN-short-title.md` with date, who decided, options, reasoning, impact and evidence, and link the PR.

## Confirmed by the owner

- Daily reading uses meanings the reader chose and reviewed, covering work, money and love.
- Multi-card reading is past / present / future with a planned 4th card that expands the future. Answers follow the cards and question and minimise guessing about personal life.
- Capture the reader's method and voice from audio and text; keep original and approved versions separate. Drafts and holdout never become approved knowledge automatically.
- An owner dashboard for the website and channels; later, create tarot video series from it.
- Paid provider usage needs a price check and approval first. Generating and publishing are separate approvals.
- **2026-11-14:** repo restructured into one monorepo with five folders (`frontend-client`, `frontend-admin`, `docs`, `backend-client`, `backend-admin`). Frontend and backend communicate only through contracts in `docs/api/`.
- **2026-11-20:** the two backend folders were merged into one `backend/` workspace (`backend/client`, `backend/admin`, shared `backend/migrations`). Stack: Rust + axum on Cloudflare Workers, D1 and R2 (backend decisions D01–D04).
- **2026-11-24:** the backend friend stopped; the owner maintains frontend and backend. Contract changes no longer need a second approver, but still go into `docs/api/` first.
- **2026-11-19:** the 3+1 reading uses reader-rated card scores (1–10 per card × category, intent overrides), weighted by position, with the 4th card acting as amplify / confirm / cap / turn_after. See [09](09-reading-rules-v0.md) and [10](10-reading-master-data.md). Case 01 scores confirmed by the reader.

## Open

| ID | Topic | Starting proposal | Status |
|---|---|---|---|
| ADR-01 | App boundary / domain / auth | Customer and admin as separate apps; add APIs in testable PRs | Open |
| ADR-02 | Engine / LLM provider | Rules (A) first, then compare B; choose provider from evidence and cost | Open |
| ADR-03 | Job queue / worker | Durable queue with one worker; choose the service after the template pilot | Open |
| ADR-04 | Customer DB / knowledge schema | Postgres with RLS + versioned releases; logical model in [04](04-data-and-payments.md) | Open |
| ADR-05 | Payments / packages | Credit packs, verified webhooks, ledger; check provider eligibility and fees | Open |
| ADR-06 | Deploy / CI / rollback | Separate staging, clean-clone builds, deploy tied to a commit | Open |
| ADR-07 | Latest review data | Owner confirms current approved count before a release | Needs data access |
| ADR-08 | Missing meanings / 4 missing card images | Reader fills and approves from source; never generated | Waiting on content |
| ADR-09 | Backend stack | Rust + axum on Cloudflare Workers, D1, R2 | Decided 2026-11-15 (backend D01, D02) |
| ADR-10 | Keep Supabase or move | New backend uses D1; the old workshop stays on Supabase as legacy until its data is migrated | Decided for new code; workshop migration open |

Proposals in this repo are not approval to use a paid provider or to change production.
