//! Builds the master-data seed SQL from `seed/*.json` (SPEC-002a, D07).
//!
//! The four seed files have the shape of the four public `client` responses. `build_sql` rejects
//! dropped provenance fields, unknown fields and control characters, checks cross-file references,
//! and renders one `INSERT ... ON CONFLICT(<key>) DO UPDATE SET` statement per row, parents before
//! children. It never emits `DELETE`, `INSERT OR REPLACE` or a write to `audit_events`.

mod model;
mod sql;
mod validate;

use serde_json::Value;
use std::fs;
use std::path::Path;

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

/// Seed file names inside the seed directory.
pub const SEED_FILES: [&str; 4] = [
    "reading-config.json",
    "cards.json",
    "card-art.json",
    "daily.json",
];

/// Text that must never appear in a seed file: ChatGPT links and the dropped fields of
/// SPEC-002 section 4.2, written as JSON keys.
pub const FORBIDDEN_TOKENS: [&str; 14] = [
    "chatgpt.com",
    "\"source_url\"",
    "\"source_message\"",
    "\"source_line\"",
    "\"review_status\"",
    "\"details_status\"",
    "\"source\"",
    "\"raw_file\"",
    "\"sourceFile\"",
    "\"sourceSha256\"",
    "\"capturedAt\"",
    "\"selection\"",
    "\"duplicates\"",
    "\"ignored\"",
];

/// Raw text of the four seed files.
#[derive(Debug, Clone)]
pub struct SeedSources {
    pub reading_config: String,
    pub cards: String,
    pub card_art: String,
    pub daily: String,
}

impl SeedSources {
    /// Reads the four seed files from `dir`.
    pub fn read_dir(dir: &Path) -> Result<Self, String> {
        let read = |name: &str| {
            let path = dir.join(name);
            fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
        };
        Ok(Self {
            reading_config: read(SEED_FILES[0])?,
            cards: read(SEED_FILES[1])?,
            card_art: read(SEED_FILES[2])?,
            daily: read(SEED_FILES[3])?,
        })
    }

    fn files(&self) -> [(&'static str, &str); 4] {
        [
            (SEED_FILES[0], &self.reading_config),
            (SEED_FILES[1], &self.cards),
            (SEED_FILES[2], &self.card_art),
            (SEED_FILES[3], &self.daily),
        ]
    }
}

/// Validates the seed and returns one SQL statement per row, in load order.
pub fn build_statements(sources: &SeedSources) -> Result<Vec<String>, String> {
    for (name, text) in sources.files() {
        check_forbidden_tokens(name, text)?;
        check_control_characters(name, text)?;
    }
    let seed = model::Seed::parse(sources)?;
    validate::validate(&seed)?;
    Ok(sql::render(&seed))
}

/// The complete `seed.sql` text: a comment header, then one statement per line group.
pub fn render_sql(statements: &[String]) -> String {
    let mut out = String::from(sql::HEADER);
    for statement in statements {
        out.push_str(statement);
        out.push('\n');
    }
    out
}

/// Validates the seed and returns the complete `seed.sql` text.
pub fn build_sql(sources: &SeedSources) -> Result<String, String> {
    Ok(render_sql(&build_statements(sources)?))
}

fn check_forbidden_tokens(name: &str, text: &str) -> Result<(), String> {
    match FORBIDDEN_TOKENS.iter().find(|token| text.contains(**token)) {
        Some(token) => Err(format!(
            "seed/{name} contains the dropped field or provenance {token} (SPEC-002 section 4.2)"
        )),
        None => Ok(()),
    }
}

/// Rejects control characters other than newline in any string (SPEC-002 section 4.5).
fn check_control_characters(name: &str, text: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("seed/{name}: {e}"))?;
    let mut stack = vec![&value];
    while let Some(value) = stack.pop() {
        match value {
            Value::String(s) => {
                if let Some(c) = s.chars().find(|c| c.is_control() && *c != '\n') {
                    return Err(format!(
                        "seed/{name}: control character U+{:04X} in {s:?}",
                        u32::from(c)
                    ));
                }
            }
            Value::Array(items) => stack.extend(items),
            Value::Object(map) => stack.extend(map.values()),
            _ => {}
        }
    }
    Ok(())
}
