//! Cross-file checks that serde cannot express. Each error names the file and the record.

use crate::model::{Seed, TextByStatus};
use std::collections::{BTreeSet, HashMap};

pub fn validate(seed: &Seed) -> Result<(), String> {
    let love_keys = love_status_keys(seed)?;
    reading_config(seed)?;
    let card_art = cards(seed, &love_keys)?;
    card_art_entries(seed, &card_art)?;
    daily(seed, &love_keys, &card_art)
}

fn unique<'a>(
    file: &str,
    what: &str,
    keys: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<&'a str>, String> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            return Err(format!("seed/{file}: duplicate {what} `{key}`"));
        }
    }
    Ok(seen)
}

fn love_status_keys(seed: &Seed) -> Result<BTreeSet<&str>, String> {
    let config = &seed.reading_config;
    let keys = unique(
        "reading-config.json",
        "love status",
        config.love_statuses.iter().map(|s| s.key.as_str()),
    )?;
    let daily_keys = unique(
        "daily.json",
        "love status",
        seed.daily.love_statuses.iter().map(|s| s.key.as_str()),
    )?;
    if keys.is_empty() {
        return Err("seed/reading-config.json: love_statuses is empty".to_string());
    }
    if keys != daily_keys {
        return Err(format!(
            "love statuses differ: reading-config.json has {keys:?}, daily.json has {daily_keys:?}"
        ));
    }
    for status in &seed.daily.love_statuses {
        let same_label = config
            .love_statuses
            .iter()
            .any(|s| s.key == status.key && s.daily_label == status.source_label);
        if !same_label {
            return Err(format!(
                "love status `{}`: daily.json source_label differs from reading-config.json daily_label",
                status.key
            ));
        }
    }
    Ok(keys)
}

fn same_keys(
    file: &str,
    record: &str,
    field: &str,
    map: &TextByStatus,
    love_keys: &BTreeSet<&str>,
) -> Result<(), String> {
    let keys: BTreeSet<&str> = map.keys().map(String::as_str).collect();
    if keys == *love_keys {
        Ok(())
    } else {
        Err(format!(
            "seed/{file}: {record}.{field} keys {keys:?} differ from the love statuses {love_keys:?}"
        ))
    }
}

fn reading_config(seed: &Seed) -> Result<(), String> {
    const FILE: &str = "reading-config.json";
    let config = &seed.reading_config;
    let categories = unique(
        FILE,
        "category",
        config.categories.iter().map(|c| c.key.as_str()),
    )?;
    unique(
        FILE,
        "position",
        config.positions.iter().map(|p| p.key.as_str()),
    )?;
    if !config.uncategorized_situations.is_empty() || !config.uncategorized_suggestions.is_empty() {
        return Err(format!(
            "seed/{FILE}: uncategorized_situations and uncategorized_suggestions must be empty in the seed"
        ));
    }
    for category in config.situations.keys().chain(config.suggestions.keys()) {
        if !categories.contains(category.as_str()) {
            return Err(format!(
                "seed/{FILE}: situations or suggestions use unknown category `{category}`"
            ));
        }
    }
    // situations.love is derived from love_statuses (key, reading_label) and is not stored.
    let derived: Vec<(&str, &str)> = config
        .love_statuses
        .iter()
        .map(|s| (s.key.as_str(), s.reading_label.as_str()))
        .collect();
    let listed: Option<Vec<(&str, &str)>> = config.situations.get("love").map(|list| {
        list.iter()
            .map(|s| (s.key.as_str(), s.label.as_str()))
            .collect()
    });
    match (categories.contains("love"), listed) {
        (true, Some(listed)) if listed == derived => {}
        (false, None) => {}
        _ => {
            return Err(format!(
            "seed/{FILE}: situations.love must equal love_statuses (key, reading_label) in order, \
                 and exist only when the category `love` exists"
        ))
        }
    }
    unique(
        FILE,
        "situation",
        config
            .situations
            .iter()
            .filter(|(category, _)| category.as_str() != "love")
            .flat_map(|(_, list)| list.iter().map(|s| s.key.as_str())),
    )?;
    unique(
        FILE,
        "suggestion id",
        config.suggestions.values().flatten().map(|s| s.id.as_str()),
    )?;
    Ok(())
}

/// Returns card id to art path (`None` for a card without art).
fn cards<'a>(
    seed: &'a Seed,
    love_keys: &BTreeSet<&str>,
) -> Result<HashMap<&'a str, Option<&'a str>>, String> {
    const FILE: &str = "cards.json";
    unique(FILE, "card id", seed.cards.iter().map(|c| c.id.as_str()))?;
    for card in &seed.cards {
        same_keys(FILE, &card.id, "meanings", &card.meanings, love_keys)?;
        same_keys(FILE, &card.id, "details", &card.details, love_keys)?;
    }
    Ok(seed
        .cards
        .iter()
        .map(|c| (c.id.as_str(), c.art.as_deref()))
        .collect())
}

/// D09 item 5: an art entry's `art` equals its card's `art`; a card without art has no entry.
fn card_art_entries(seed: &Seed, card_art: &HashMap<&str, Option<&str>>) -> Result<(), String> {
    const FILE: &str = "card-art.json";
    unique(
        FILE,
        "art entry",
        seed.card_art.cards.iter().map(|a| a.id.as_str()),
    )?;
    for entry in &seed.card_art.cards {
        match card_art.get(entry.id.as_str()) {
            None => return Err(format!("seed/{FILE}: art entry `{}` has no card", entry.id)),
            Some(art) if *art != Some(entry.art.as_str()) => {
                return Err(format!(
                    "seed/{FILE}: art entry `{}` art differs from the card's art {art:?}",
                    entry.id
                ))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn daily(
    seed: &Seed,
    love_keys: &BTreeSet<&str>,
    cards: &HashMap<&str, Option<&str>>,
) -> Result<(), String> {
    const FILE: &str = "daily.json";
    unique(
        FILE,
        "daily card",
        seed.daily.cards.iter().map(|d| d.card_id.as_str()),
    )?;
    for entry in &seed.daily.cards {
        if !cards.contains_key(entry.card_id.as_str()) {
            return Err(format!(
                "seed/{FILE}: daily card `{}` has no card",
                entry.card_id
            ));
        }
        same_keys(
            FILE,
            &entry.card_id,
            "content.love_by_status",
            &entry.content.love_by_status,
            love_keys,
        )?;
        let expected: Vec<String> = seed
            .daily
            .love_statuses
            .iter()
            .filter(|s| matches!(entry.content.love_by_status.get(&s.key), Some(None)))
            .map(|s| format!("content.love_by_status.{}", s.key))
            .collect();
        if entry.missing_fields != expected {
            return Err(format!(
                "seed/{FILE}: daily card `{}` missing_fields {:?} differ from the null texts {expected:?}",
                entry.card_id, entry.missing_fields
            ));
        }
    }
    Ok(())
}
