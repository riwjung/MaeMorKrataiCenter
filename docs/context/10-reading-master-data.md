# 10 · Reading master data (proposal for backend/admin)

Status: **proposal** (2026-11-19). Supports the rules in [09 · Reading rules v0](09-reading-rules-v0.md). Written as Cloudflare D1 (SQLite) DDL in the same style as `backend/migrations/0001_init_auth.sql` (TEXT ids, INTEGER epoch timestamps, CHECK enums). Canonical migrations live in `backend/migrations/` and are applied through the admin Worker; adapt freely.

## Principles

- **Versioned knowledge.** Every rule row belongs to a `knowledge_release`. Readings store the release they used, so editing data never changes a reading a customer already received.
- **Draft vs approved.** Rows carry `status`; only rows in a `published` release are visible to the client Worker.
- **Canonical keys.** English keys everywhere (`love`, `commitment`, `talking`, `the-fool`); Thai labels are display data.
- **No real answers in the rule tables.** Reader answers and audio stay in private storage; rules link to them by `source_ref` only.

## Tables

### Reference lists

```sql
CREATE TABLE categories (
  key TEXT PRIMARY KEY,                -- love | work | money | health
  label_th TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE intents (
  key TEXT PRIMARY KEY,                -- commitment | breakup | reconcile | feelings | direction | choice | clarify
  label_th TEXT NOT NULL,
  polarity TEXT NOT NULL CHECK (polarity IN ('high_is_yes', 'high_is_no', 'none')),
  answer_mode TEXT NOT NULL CHECK (answer_mode IN ('verdict', 'advice'))   -- v0.2, Rule 6
);

CREATE TABLE situations (
  key TEXT PRIMARY KEY,                -- single | crush | talking | partnered | ex_partner | complicated | ...
  category_key TEXT NOT NULL REFERENCES categories (key),
  label_th TEXT NOT NULL
);

CREATE TABLE spread_positions (
  key TEXT PRIMARY KEY,                -- past | present | future | expansion
  label_th TEXT NOT NULL,
  sort_order INTEGER NOT NULL
);

CREATE TABLE cards (
  id TEXT PRIMARY KEY,                 -- the-fool, seven-of-cups ... (same IDs as the website)
  name_en TEXT NOT NULL,
  arcana TEXT NOT NULL CHECK (arcana IN ('major', 'minor')),
  suit TEXT CHECK (suit IN ('wands', 'cups', 'swords', 'pentacles')),
  art_key TEXT                         -- R2 object key, null if art is missing
);
```

### Releases

```sql
CREATE TABLE knowledge_releases (
  id TEXT PRIMARY KEY,                 -- e.g. rules-v0.1
  status TEXT NOT NULL CHECK (status IN ('draft', 'in_review', 'published', 'retired')),
  notes TEXT,
  approved_by TEXT REFERENCES users (id),
  approved_at INTEGER,
  created_at INTEGER NOT NULL
);
```

### Rule data (Rules 1–3)

```sql
-- Rule 1: one row per card x category (312 rows per release)
CREATE TABLE card_scores (
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  card_id TEXT NOT NULL REFERENCES cards (id),
  category_key TEXT NOT NULL REFERENCES categories (key),
  score INTEGER NOT NULL CHECK (score BETWEEN 1 AND 10),
  keyword_th TEXT NOT NULL,
  meaning_th TEXT NOT NULL,
  condition_th TEXT,
  person_trait_th TEXT,
  advice_th TEXT,                      -- v0.2: what the card tells the person to do (advice mode)
  status TEXT NOT NULL CHECK (status IN ('draft', 'reader_confirmed')),
  source_ref TEXT,                     -- e.g. workshop case id, never the answer text
  updated_by TEXT REFERENCES users (id),
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (release_id, card_id, category_key)
);

-- Rule 1: only where the reader reads a card differently for one intent
CREATE TABLE card_intent_overrides (
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  card_id TEXT NOT NULL REFERENCES cards (id),
  category_key TEXT NOT NULL REFERENCES categories (key),
  intent_key TEXT NOT NULL REFERENCES intents (key),
  score INTEGER NOT NULL CHECK (score BETWEEN 1 AND 10),
  meaning_th TEXT NOT NULL,
  condition_th TEXT,
  status TEXT NOT NULL CHECK (status IN ('draft', 'reader_confirmed')),
  source_ref TEXT,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (release_id, card_id, category_key, intent_key)
);

-- Rule 2: weights per release (optionally per intent)
CREATE TABLE position_weights (
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  intent_key TEXT NOT NULL DEFAULT '*',    -- '*' = default for all intents
  position_key TEXT NOT NULL REFERENCES spread_positions (key),
  weight_bp INTEGER NOT NULL CHECK (weight_bp BETWEEN 0 AND 10000),  -- basis points, 2000 = 20%
  PRIMARY KEY (release_id, intent_key, position_key)
);

-- Rule 2: answer ladder per category x intent
CREATE TABLE answer_ladders (
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  category_key TEXT NOT NULL REFERENCES categories (key),
  intent_key TEXT NOT NULL REFERENCES intents (key),
  level INTEGER NOT NULL CHECK (level BETWEEN 1 AND 9),
  min_total_x100 INTEGER NOT NULL,         -- 400 = 4.00, inclusive
  max_total_x100 INTEGER NOT NULL,         -- exclusive
  answer_th TEXT NOT NULL,                 -- the plain verdict sentence
  status TEXT NOT NULL CHECK (status IN ('draft', 'reader_confirmed')),
  PRIMARY KEY (release_id, category_key, intent_key, level)
);

-- Rule 3: thresholds for expansion roles
CREATE TABLE expansion_role_rules (
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  role TEXT NOT NULL CHECK (role IN ('amplify', 'confirm', 'cap', 'turn_after', 'advise')),
  min_delta INTEGER,                       -- expansion score minus future score
  max_delta INTEGER,
  crosses_neutral INTEGER NOT NULL CHECK (crosses_neutral IN (0, 1)),
  ladder_shift INTEGER NOT NULL DEFAULT 0, -- +1 amplify, 0 otherwise (cap only changes wording)
  template_th TEXT NOT NULL,               -- phrasing for step 4 of the answer
  PRIMARY KEY (release_id, role)
);

-- Rule 4: answer templates per step
CREATE TABLE answer_templates (
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  step TEXT NOT NULL CHECK (step IN ('past', 'present', 'future', 'expansion', 'verdict', 'advice')),
  category_key TEXT NOT NULL DEFAULT '*',
  intent_key TEXT NOT NULL DEFAULT '*',
  expands_position TEXT,                   -- v0.2 advice mode: which position the 4th card expands (default 'future')
  body_th TEXT NOT NULL,                   -- placeholders: {card} {keyword} {meaning} {condition} {trait} {advice} {answer}
  PRIMARY KEY (release_id, step, category_key, intent_key)
);
```

### Readings and traces (customer side)

```sql
CREATE TABLE reading_sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users (id),      -- null until customer accounts exist
  category_key TEXT NOT NULL REFERENCES categories (key),
  intent_key TEXT NOT NULL REFERENCES intents (key),
  situation_key TEXT REFERENCES situations (key),
  question TEXT,                           -- personal data; deletable by the user
  stage TEXT NOT NULL CHECK (stage IN ('baseline', 'followup', 'done')),
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  idempotency_key TEXT UNIQUE,
  created_at INTEGER NOT NULL
);

CREATE TABLE reading_cards (
  session_id TEXT NOT NULL REFERENCES reading_sessions (id),
  position_key TEXT NOT NULL REFERENCES spread_positions (key),
  card_id TEXT NOT NULL REFERENCES cards (id),
  revealed_at INTEGER,                     -- expansion stays null until the follow-up
  PRIMARY KEY (session_id, position_key)
);

CREATE TABLE reading_outputs (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES reading_sessions (id),
  stage TEXT NOT NULL CHECK (stage IN ('baseline', 'followup')),
  status TEXT NOT NULL CHECK (status IN ('answered', 'clarify', 'needs_owner', 'out_of_scope')),
  engine TEXT NOT NULL CHECK (engine IN ('A', 'B', 'reader')),
  answer_th TEXT,
  trace_json TEXT NOT NULL,                -- scores used, weights, total, role, ladder level, row keys
  created_at INTEGER NOT NULL
);
```

### Evaluation (development / holdout)

```sql
CREATE TABLE eval_runs (
  id TEXT PRIMARY KEY,
  release_id TEXT NOT NULL REFERENCES knowledge_releases (id),
  split TEXT NOT NULL CHECK (split IN ('development', 'holdout')),
  engine TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE eval_results (
  run_id TEXT NOT NULL REFERENCES eval_runs (id),
  case_id TEXT NOT NULL,                   -- workshop case id
  expected_level INTEGER,                  -- reader's verdict mapped to the ladder
  predicted_level INTEGER,
  expected_role TEXT,
  predicted_role TEXT,
  reader_verdict TEXT CHECK (reader_verdict IN ('accept', 'edit', 'reject')),
  PRIMARY KEY (run_id, case_id)
);
```

## Engine A in one function

```
load release → for each drawn card: score = override(card, category, intent) ?? card_scores(card, category)
  any score missing           → needs_owner
  role  = expansion_role_rules(expansion.score - future.score, crosses 5?)   -- turn_after checked first
  total = role == turn_after ? mean(past, present, future) : Σ score × weight(position, intent)
  level = min(top, answer_ladders(category, intent, polarity-adjusted total) + role.ladder_shift)
  text  = fill answer_templates(step, category, intent) with card rows, role template, ladder answer
  store reading_outputs with trace_json
```

## API surface (to agree in `docs/api/`)

- `backend/admin`: CRUD for `card_scores`, `card_intent_overrides`, `answer_ladders`, `answer_templates`; release create / submit / publish; run evaluation.
- `backend/client`: start session (baseline), reveal expansion (follow-up), get output. Only published releases.
