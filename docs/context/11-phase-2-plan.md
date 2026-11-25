# 11 · Phase 2 plan: 3+1 reading engine (rules)

Status: **plan** (2026-11-19). Phase 1 (static site, Cloudflare deploy, backend scaffold) is done. Phase 2 makes the paid 3+1 reading answerable by rules (Engine A), with the reader in control of every number.

Rules: [09](09-reading-rules-v0.md) · Data model: [10](10-reading-master-data.md).

## Findings from the 40 development cases (2026-11-19)

All 40 development answers were read locally (answers stay private). Three findings change the rules:

1. **Two answer modes.** About two thirds of the questions are "how should I …" (`direction`, `clarify`). For those the reader does not give a verdict; she gives advice, and the 4th card **adds a concrete action** ("the 4th card expands card 3: …"). Only yes/no questions (`commitment`, `breakup`, `reconcile`, "will he come back") use the score ladder and the expansion roles from Rule 3.
2. **The 4th card targets a card.** In advice mode the reader often says which card the 4th card expands (card 3 most often, sometimes cards 1 and 3, or the 4th card itself). The answer template needs an `expands_position` value.
3. **Cases come in pairs.** Most development cases share the same 3 cards and differ only in the 4th card. These pairs are a ready-made test set for Rule 3 (e.g. Justice vs The Emperor; Temperance vs Four of Swords; Queen of Swords with Six of Swords vs Two of Cups).

### Rule changes for v0.2

| Change | Detail |
|---|---|
| `intents.answer_mode` | `verdict` (ladder + expansion role) or `advice` (actions, no ladder) |
| Advice step | 4th card meaning in the category, phrased as an action, attached to the position it expands |
| `card_scores.advice_th` | New short field: what the card tells the person to do in this category |
| Verdict line | Only in `verdict` mode; `advice` mode ends with the main action instead |

## Work items

| # | Item | Part | Output | Done when |
|---|---|---|---|---|
| 1 | Rules v0.2 spec | Frontend | Update 09 and 10 with the changes above | 09 and 10 agree with each other and with the case-01–03 examples |
| 2 | Draft card data | Frontend + reader | 78 × 4 rows: score, keyword, meaning, condition, advice. Drafted from the reader's daily meanings and development answers, every row `draft` | Reader has confirmed or edited each row (sheet first) |
| 3 | Expected answers for evaluation | Frontend | For each development case: mode, expected ladder level or main action, expected expansion role | Labels checked by the reader on a sample |
| 4 | Reference engine + evaluator | Frontend | Small reference implementation of Engine A and a local evaluator over the 40 cases; only aggregate results leave the machine | ≥ 80 % of verdict cases hit the reader's ladder level and role; advice cases name the reader's main action |
| 5 | Master data migration + admin API | Backend | D1 tables from 10, CRUD for card data, release publish | Reader data can be loaded and versioned |
| 6 | Reading API contract | Both | `docs/api/client/openapi.yaml`: start session (3 cards), reveal 4th card, get answer; `docs/api/admin/openapi.yaml`: card data + releases | Contract merged before the endpoints are built |
| 7 | Engine A in backend/client | Backend | Rust port of the reference engine, same test vectors | Same outputs as the reference on all test vectors |
| 8 | Customer UI for 3+1 | Frontend | `/reading` gets the 4th card step, calls the API (mock first) | Works against the mock, then staging |
| 9 | Reader scoring page | Frontend | `frontend-admin` page to edit card data | Reader can edit without a sheet |
| 10 | Holdout test | Both | One run on the 20 holdout cases with the locked release | Pilot targets in [07](07-roadmap.md) |

Order: 1 → 2 + 3 → 4 → 6 → (5, 7 in parallel with 8) → 9 → 10.

## Out of Phase 2

Payments and accounts (Phase 3), LLM rewriting (Engine B), timing predictions, reversed cards, card-pair rules.

## Explainer clip

A 2-minute vertical clip that walks through rules v0 with the real v3 cards (case 01 / 02 / 03) exists for alignment with the reader and the backend. Ask the frontend owner for the file; it is not stored in Git.
