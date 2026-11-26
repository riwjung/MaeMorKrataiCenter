# Database Architecture & Migration Policy

## 1. Engine & Ownership
- **Database Engine**: Cloudflare D1 (SQLite-compatible serverless SQL)
- **Binding Name**: `DB`
- **Canonical Migrations Owner**: `backend-admin/migrations/` (Decision `D01`, `D02`)
  - Both `backend-admin` and `backend-client` bind to the shared D1 database per environment, but all schema migrations MUST be authored and applied exclusively from `backend-admin/migrations/`.

## 2. Schema (`migrations/0001_init_auth.sql`)

### `users`
Stores admin/owner/user accounts (self-service HTTP signup is disabled; accounts are provisioned via `make create-user`).
- `id` (`TEXT PRIMARY KEY`), `email` (`TEXT NOT NULL UNIQUE`), `password_hash` (`TEXT NOT NULL`), `role` (`TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'user'))`), `is_active` (`INTEGER NOT NULL DEFAULT 1`), `created_at` (`INTEGER NOT NULL`), `updated_at` (`INTEGER NOT NULL`)
- Index: `idx_users_email` on `(email)`

### `sessions`
Stores opaque refresh token SHA-256 hashes with rotation lineage for reuse/replay detection.
- `id` (`TEXT PRIMARY KEY`), `user_id` (`TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE`), `token_hash` (`TEXT NOT NULL UNIQUE`), `family_id` (`TEXT NOT NULL`), `expires_at` (`INTEGER NOT NULL`), `revoked_at` (`INTEGER`), `replaced_by_session_id` (`TEXT`), `created_at` (`INTEGER NOT NULL`)
- Indexes: `idx_sessions_user_id`, `idx_sessions_family_id`, `idx_sessions_token_hash`

### `jwks_keys`
Stores `ES256`/`RS256` public JWKs and AES-256-GCM envelope-encrypted private keys.
- `kid` (`TEXT PRIMARY KEY`), `alg` (`TEXT NOT NULL CHECK (alg IN ('ES256', 'RS256'))`), `public_jwk_json` (`TEXT NOT NULL`), `encrypted_private_key` (`TEXT NOT NULL`), `status` (`TEXT NOT NULL CHECK (status IN ('active', 'retiring', 'revoked'))`), `created_at` (`INTEGER NOT NULL`), `expires_at` (`INTEGER`)
- Index: `idx_jwks_keys_status` on `(status)`

### `audit_events`
Stores security and administrative audit trail events.
- `id` (`TEXT PRIMARY KEY`), `actor_user_id` (`TEXT`), `event_type` (`TEXT NOT NULL`), `target_id` (`TEXT`), `metadata_json` (`TEXT NOT NULL`), `created_at` (`INTEGER NOT NULL`)
- Indexes: `idx_audit_events_created_at`, `idx_audit_events_event_type`

### Master data (`migrations/0002_master_data.sql`)
Migration 0002 adds 12 master-data tables: `love_statuses`, `categories`, `situations`, `suggestions`, `positions`, `cards`, `card_meanings`, `card_details`, `card_art`, `daily_meta`, `daily_cards` and `daily_love_texts`. Rows come from `seed/*.json` through `make seed-*`, not from the migration. Columns, keys and rules are in [features/master-data.md](../../knowledges/features/master-data.md).

## 3. Migration Commands
- Local: `make migrate-local` (`npx wrangler d1 migrations apply DB --local`)
- Staging: `make migrate-staging` (`npx wrangler d1 migrations apply DB --env staging --remote`)
- Production: `make migrate-prod` (`npx wrangler d1 migrations apply DB --env prod --remote`)
