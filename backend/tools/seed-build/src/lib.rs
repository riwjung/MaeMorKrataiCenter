//! Seed build for the SPEC-002a master data (D07).

/// Master-data tables in load order: every parent table comes before its children.
pub const TABLE_ORDER: [&str; 12] = [
    "love_statuses",
    "categories",
    "situations",
    "suggestions",
    "positions",
    "cards",
    "card_meanings",
    "card_details",
    "card_art",
    "daily_meta",
    "daily_cards",
    "daily_love_texts",
];
