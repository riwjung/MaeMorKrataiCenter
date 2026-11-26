-- Canonical shared D1 schema migration 0002: master data (SPEC-002a; SPEC-002 sections 4.3 and 4.4).
-- DDL only (D07): rows come from seed/*.json through `make seed-local|seed-staging|seed-prod`.
-- Foreign keys take no delete or update action (D09). D1 enforces foreign keys on every statement.
-- `category` is nullable on situations and suggestions: NULL means uncategorized (D10).
-- Love situations are not stored; they are derived from love_statuses (SPEC-002 section 4.3).
-- Booleans are INTEGER 0/1. Sort orders are per screen; ties are broken by key or id when read.

CREATE TABLE IF NOT EXISTS love_statuses (
    key TEXT PRIMARY KEY NOT NULL,
    daily_label TEXT NOT NULL,
    reading_label TEXT NOT NULL,
    daily_sort_order INTEGER NOT NULL,
    reading_sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS categories (
    key TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    icon TEXT NOT NULL,
    hint TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS situations (
    key TEXT PRIMARY KEY NOT NULL,
    category TEXT REFERENCES categories (key),
    label TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_situations_category ON situations (category);

CREATE TABLE IF NOT EXISTS suggestions (
    id TEXT PRIMARY KEY NOT NULL,
    category TEXT REFERENCES categories (key),
    text TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_suggestions_category ON suggestions (category);

CREATE TABLE IF NOT EXISTS positions (
    key TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    keyword TEXT NOT NULL,
    art TEXT,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS card_meanings (
    card_id TEXT NOT NULL REFERENCES cards (id),
    love_status TEXT NOT NULL REFERENCES love_statuses (key),
    text TEXT,
    PRIMARY KEY (card_id, love_status)
);

CREATE INDEX IF NOT EXISTS idx_card_meanings_love_status ON card_meanings (love_status);

CREATE TABLE IF NOT EXISTS card_details (
    card_id TEXT NOT NULL REFERENCES cards (id),
    love_status TEXT NOT NULL REFERENCES love_statuses (key),
    text TEXT,
    PRIMARY KEY (card_id, love_status)
);

CREATE INDEX IF NOT EXISTS idx_card_details_love_status ON card_details (love_status);

CREATE TABLE IF NOT EXISTS card_art (
    card_id TEXT PRIMARY KEY NOT NULL REFERENCES cards (id),
    art TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    rendered_width INTEGER NOT NULL,
    rendered_height INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    sha256 TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS daily_meta (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    schema_version INTEGER NOT NULL,
    version TEXT NOT NULL,
    updated TEXT NOT NULL,
    status TEXT NOT NULL,
    scope TEXT NOT NULL,
    auto_fallback_to_general_or_spread INTEGER NOT NULL CHECK (auto_fallback_to_general_or_spread IN (0, 1))
);

CREATE TABLE IF NOT EXISTS daily_cards (
    card_id TEXT PRIMARY KEY NOT NULL REFERENCES cards (id),
    name TEXT NOT NULL,
    arcana TEXT NOT NULL,
    reading_mode TEXT NOT NULL,
    time_horizon TEXT NOT NULL,
    orientation TEXT NOT NULL,
    content_status TEXT NOT NULL,
    approved_for_web INTEGER NOT NULL CHECK (approved_for_web IN (0, 1)),
    theme TEXT NOT NULL,
    overview TEXT NOT NULL,
    work TEXT NOT NULL,
    money TEXT NOT NULL,
    rabbit_message TEXT NOT NULL,
    colors TEXT NOT NULL,
    affirmation TEXT NOT NULL,
    summary_theme TEXT,
    overview_theme TEXT,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS daily_love_texts (
    card_id TEXT NOT NULL REFERENCES daily_cards (card_id),
    love_status TEXT NOT NULL REFERENCES love_statuses (key),
    text TEXT,
    PRIMARY KEY (card_id, love_status)
);

CREATE INDEX IF NOT EXISTS idx_daily_love_texts_love_status ON daily_love_texts (love_status);
