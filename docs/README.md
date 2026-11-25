# docs

The shared space between frontend and backend. Read `context/` first, then agree contracts in `api/`.

| Path | Contents |
|---|---|
| [`context/`](context) | Project knowledge, translated from the old repo: overview, current state, architecture, data and payments, reading engine, admin/Content Studio, roadmap, open decisions, glossary, **reading rules v0 (09)** and **reading master data proposal (10)** |
| [`reference/`](reference) | Raw reference material: legacy workshop SQL migrations, the never-migrated schema proposal (DBML), Social Center summary |
| `api/client/` | API contract for the customer website. Use OpenAPI (`openapi.yaml`). |
| `api/admin/` | API contract for the admin dashboard and content automation. |
| `decisions/` | Project-level decision records, named `NNNN-short-title.md`. Backend technical decisions (D01–D10) live in [`../backend/docs/`](../backend/docs). |
| [`handover-frontend-decoupling.md`](handover-frontend-decoupling.md) | Audit of `frontend-client/` data/features to move to backend & handover bridge to Spec 01 (Rust + Cloudflare Workers/D1/R2 scaffold). |

Start with [`context/01-overview.md`](context/01-overview.md).

## Workflow

One person maintains frontend and backend since 2026-11-24, so there is no cross-team approval step. The contract still comes first:

1. Need a new or changed endpoint? Edit `api/` first, on a branch.
2. Implement the backend and the frontend call against that contract, in the same pull request or a later one.
3. Once merged, the contract is the source of truth: the frontend can mock against it and the backend tests check it.

## Source of the context docs

Translated and condensed on 2026-11-14 from the old repo `riwjung/MaeMorKratai`: `docs/consult/*` (2026-11-05), `backend/READING_ENGINE_PLAN.md` (2026-10-26), `backend/db/maemorkratai.dbml` (2026-10-17), `services/social-center/README.md`. Dated status notes, local machine paths and private project IDs were removed. Real reader answers, audio and holdout data are not in this repo and must never be added.
