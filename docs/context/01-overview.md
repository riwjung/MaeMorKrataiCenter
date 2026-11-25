# 01 · Project overview

Last reviewed: 2026-11-14. Translated and condensed from the old repo (`riwjung/MaeMorKratai`, `docs/consult/` dated 2026-11-05).

## What MaeMorKratai is

MaeMorKratai (แม่หมอกระต่าย, "the rabbit fortune teller") is a Thai tarot brand: a website where customers draw cards, plus short-form video channels. The fortune-telling knowledge and writing voice come from the owner's partner, who is the real reader ("the reader" in these docs). The site must never invent fortune-telling methods or predictions; content is the reader's, reviewed by the reader.

## Product goals (confirmed by the owner)

1. **Daily reading** uses reader-approved meanings, covering work, money and love (not love only).
2. **Multi-card reading** is past / present / future, with a planned **4th card that expands the future**. Answers must be based on the cards and the question, and avoid guessing about the customer's personal life.
3. **Capture the reader's method and voice** from her audio and text answers. Keep the original and the approved version separately. Drafts and holdout answers are never treated as approved knowledge.
4. **An admin dashboard** to run the website and the content channels. Later, **create tarot video series from the dashboard**.
5. **Any paid provider usage** (AI image/video/voice credits) needs a price check and owner approval first. Generating content and publishing it are two separate approvals.

## Ownership (from 2026-11-24)

From 2026-11-14 to 2026-11-23 the work was split: the owner built the frontend and a friend built the backend scaffold (SPEC-001 admin auth, SPEC-002 master-data spec, Cloudflare deploy prep). The friend has stopped; since 2026-11-24 the owner maintains everything.

| Area | Folder |
|---|---|
| Customer website UI | `frontend-client/` |
| Admin dashboard UI, content automation tool | `frontend-admin/` |
| Customer API (public Worker) | `backend/client/` |
| Admin API (auth + admin Worker), D1 migrations | `backend/admin/`, `backend/migrations/` |
| Contracts and shared knowledge | `docs/` |

The content automation program (video generation pipeline) runs only on the owner's Mac and talks to the admin Worker through the admin API.

## Reading order

1. [02 · Current state](02-current-state.md): what is live today and what is demo
2. [03 · Architecture](03-architecture.md): old systems and the proposed target
3. [04 · Data and payments](04-data-and-payments.md): existing tables, proposed tables, payment flow
4. [05 · Reading engine](05-reading-engine.md): how answers should be generated
5. [06 · Admin dashboard and Content Studio](06-admin-and-content-studio.md)
6. [07 · Roadmap](07-roadmap.md)
7. [08 · Open decisions](08-open-decisions.md)
8. [Glossary](glossary.md): Thai terms, card IDs, relationship statuses
9. [09 · Reading rules v0](09-reading-rules-v0.md) and [10 · Reading master data](10-reading-master-data.md): how the 3+1 answer is computed
10. [11 · Phase 2 plan](11-phase-2-plan.md): work items for the reading engine

## Open technical questions

| Topic | Starting proposal | What needs review |
|---|---|---|
| App boundary | Customer site and admin as separate apps | When to merge auth/domain without breaking the free site |
| Reading engine | Reader-approved rules/text first, then compare with an LLM that rewrites from the same evidence | How to scope questions, positions and the 4th card, and how to measure quality |
| Database | Postgres (Supabase today) with Auth and Storage; workshop data separate from customer data | Schema, row-level security, transactions, backup/restore, dev/staging/prod |
| Payments | Credit packs per question first, subscriptions later | Provider fit for a Thai merchant, refunds, failures, webhooks |
| Dashboard | Owner sees cases, knowledge, orders, video jobs, costs | Roles and a review workflow that exposes no more data than needed |
| Video worker | Run the existing template through a queue | Mac worker for the pilot or VM/container; when a GPU is needed |
| Operations | Separate staging; deploy from a known commit | CI, rollback, logs without user questions, audio or secrets |
