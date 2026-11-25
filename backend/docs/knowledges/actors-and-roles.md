# Actors & Roles

Who uses the backend, how they authenticate, and what they may do. Applies to both services (`admin`, `client`).

Last confirmed with the backend owner: 2026-11-20.

## Actors

| Actor | Who | Service | Authenticated? | DB account? |
|---|---|---|---|---|
| **Customer** | Any member of the public using the website (`frontend-client`). | `client` | No. Anonymous, public access only (D04 defenses: rate limit, Turnstile where required). | No. No customer accounts, login, history or payments until Phase 3. |
| **Reader** (แม่หมอ) | The real tarot reader, the owner's partner. Source and approver of all fortune-telling knowledge: card scores, meanings, ladders, templates. | `admin` | Yes. Email + password login (SPEC-001, D03). | Yes, a `users` row with role `admin`. |
| **Owner** | The business owner. Runs the site and channels. | `admin` | Yes. Same login as the reader. | Yes, a `users` row with role `admin`. |

## Key points

- **The reader is not a public user.** She works through the admin Worker (`backend/admin`) with an admin account. All knowledge editing and release publishing happen on the admin side.
- **Admin is the working top role.** The owner and the reader both hold `admin` and have the same permissions (edit master data, publish releases). There is no edit vs. publish split between them.
- **Customers are anonymous.** "No account" does not mean "no stored state": a 3+1 reading still needs the server to remember the hidden 4th card between the start and reveal calls. That state is not tied to a person.
- **Accounts are provisioned, never self-registered.** Admin users are created with `make create-user` (D03).

## Open

- The `users.role` schema (D03, `migrations/0001_init_auth.sql`) also defines `owner` (above `admin`) and `user` (future customers). `POST /v1/admin/auth/keys/rotate` is restricted to `owner`. Who, if anyone, holds the `owner` role is not yet recorded.

## References

- `docs/admin/decisions/D03.md`: admin auth and role hierarchy
- `docs/admin/decisions/D04.md`: public `client` security baseline
- `../docs/context/glossary.md`: domain terms (reader, knowledge release, workshop)
