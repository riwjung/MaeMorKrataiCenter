// Seed loader and golden assertions for SPEC-002a, shared by spec_002a_seed_scenarios.rs and
// d1_master_data_sql.rs. Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use backend_client::adapters::memory::MasterDataFixture;
use backend_client::master_data::{
    CardArtRow, CardRow, CategoryRow, DailyCardRow, DailyMetaRow, LoveStatusRow, PositionRow,
    SituationRow, SuggestionRow, TextMap, LOVE_CATEGORY,
};
use serde_json::{json, Value};
use std::collections::HashMap;

pub const CARDS_JSON: &str = include_str!("../../../seed/cards.json");
pub const DAILY_JSON: &str = include_str!("../../../seed/daily.json");
pub const CARD_ART_JSON: &str = include_str!("../../../seed/card-art.json");
pub const READING_CONFIG_JSON: &str = include_str!("../../../seed/reading-config.json");
pub const SEED_FILES: [(&str, &str); 4] = [
    ("seed/cards.json", CARDS_JSON),
    ("seed/daily.json", DAILY_JSON),
    ("seed/card-art.json", CARD_ART_JSON),
    ("seed/reading-config.json", READING_CONFIG_JSON),
];

/// SPEC-002 §4.3.
pub const DAILY_LOVE_ORDER: [&str; 6] = [
    "single",
    "crush",
    "talking",
    "partnered",
    "ex_partner",
    "complicated",
];
pub const READING_LOVE_ORDER: [&str; 6] = [
    "single",
    "crush",
    "talking",
    "complicated",
    "partnered",
    "ex_partner",
];
/// Scenario 3.
pub const NO_ART_CARDS: [&str; 4] = [
    "six-of-wands",
    "nine-of-wands",
    "page-of-pentacles",
    "king-of-pentacles",
];
/// SPEC-002 §4.2 dropped fields and provenance markers; none may appear in seed or bodies.
pub const FORBIDDEN: [&str; 14] = [
    "chatgpt.com",
    "source_url",
    "source_message",
    "source_line",
    "review_status",
    "details_status",
    "raw_file",
    "sourceFile",
    "sourceSha256",
    "capturedAt",
    "\"source\":",
    "\"selection\":",
    "\"duplicates\":",
    "\"ignored\":",
];
/// Scenario A1 as direct SQL (task 07). Love suggestions are deleted first: the FK on
/// `suggestions.category` (no cascade) rejects deleting the `love` category otherwise.
pub const A1_SQL: &str = "UPDATE situations SET category = NULL WHERE category = 'work'; \
UPDATE suggestions SET category = NULL WHERE category = 'work'; \
DELETE FROM suggestions WHERE category = 'love'; \
DELETE FROM categories WHERE \"key\" IN ('work', 'love');";

pub fn seed(text: &str) -> Value {
    serde_json::from_str(text).expect("seed file is valid JSON")
}

fn str_field(v: &Value, key: &str) -> String {
    v[key]
        .as_str()
        .unwrap_or_else(|| panic!("seed: `{key}` is not a string in {v}"))
        .to_string()
}

fn opt_str_field(v: &Value, key: &str) -> Option<String> {
    match &v[key] {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => panic!("seed: `{key}` is neither a string nor null: {other}"),
    }
}

fn int_field(v: &Value, key: &str) -> i64 {
    v[key]
        .as_i64()
        .unwrap_or_else(|| panic!("seed: `{key}` is not an integer in {v}"))
}

fn bool_field(v: &Value, key: &str) -> bool {
    v[key]
        .as_bool()
        .unwrap_or_else(|| panic!("seed: `{key}` is not a boolean in {v}"))
}

fn array<'a>(v: &'a Value, what: &str) -> &'a [Value] {
    v.as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("seed: {what} is not an array"))
}

fn text_map(v: &Value, what: &str) -> TextMap {
    v.as_object()
        .unwrap_or_else(|| panic!("seed: {what} is not an object"))
        .iter()
        .map(|(key, text)| {
            let text = match text {
                Value::Null => None,
                Value::String(t) => Some(t.clone()),
                other => panic!("seed: {what}.{key} is neither a string nor null: {other}"),
            };
            (key.clone(), text)
        })
        .collect()
}

fn index(n: usize) -> i64 {
    i64::try_from(n).unwrap()
}

fn reversed<T>(mut rows: Vec<T>) -> Vec<T> {
    rows.reverse();
    rows
}

/// The seed as memory-adapter rows: `sort_order` is the file index, rows stored reversed.
pub fn seed_fixture() -> MasterDataFixture {
    let cards = seed(CARDS_JSON);
    let daily = seed(DAILY_JSON);
    let art = seed(CARD_ART_JSON);
    let config = seed(READING_CONFIG_JSON);

    let card_order: HashMap<String, i64> = array(&cards, "cards.json")
        .iter()
        .enumerate()
        .map(|(n, c)| (str_field(c, "id"), index(n)))
        .collect();
    let daily_order: HashMap<String, i64> = array(&daily["love_statuses"], "daily.love_statuses")
        .iter()
        .enumerate()
        .map(|(n, s)| (str_field(s, "key"), index(n)))
        .collect();

    let love_statuses = array(&config["love_statuses"], "reading-config.love_statuses")
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let key = str_field(s, "key");
            LoveStatusRow {
                daily_sort_order: daily_order[&key],
                reading_sort_order: index(n),
                daily_label: str_field(s, "daily_label"),
                reading_label: str_field(s, "reading_label"),
                key,
            }
        })
        .collect();
    let categories = array(&config["categories"], "reading-config.categories")
        .iter()
        .enumerate()
        .map(|(n, c)| CategoryRow {
            key: str_field(c, "key"),
            label: str_field(c, "label"),
            icon: str_field(c, "icon"),
            hint: str_field(c, "hint"),
            sort_order: index(n),
        })
        .collect();
    let situations = config["situations"]
        .as_object()
        .expect("seed: reading-config.situations is not an object")
        .iter()
        .filter(|(category, _)| category.as_str() != LOVE_CATEGORY)
        .flat_map(|(category, list)| {
            array(list, "reading-config.situations.*")
                .iter()
                .enumerate()
                .map(move |(n, s)| SituationRow {
                    key: str_field(s, "key"),
                    category: Some(category.clone()),
                    label: str_field(s, "label"),
                    sort_order: index(n),
                })
        })
        .collect();
    let suggestions = config["suggestions"]
        .as_object()
        .expect("seed: reading-config.suggestions is not an object")
        .iter()
        .flat_map(|(category, list)| {
            array(list, "reading-config.suggestions.*")
                .iter()
                .enumerate()
                .map(move |(n, s)| SuggestionRow {
                    id: str_field(s, "id"),
                    category: Some(category.clone()),
                    text: str_field(s, "text"),
                    sort_order: index(n),
                })
        })
        .collect();
    let positions = array(&config["positions"], "reading-config.positions")
        .iter()
        .enumerate()
        .map(|(n, p)| PositionRow {
            key: str_field(p, "key"),
            label: str_field(p, "label"),
            sort_order: index(n),
        })
        .collect();
    let card_rows = array(&cards, "cards.json")
        .iter()
        .enumerate()
        .map(|(n, c)| CardRow {
            id: str_field(c, "id"),
            name: str_field(c, "name"),
            keyword: str_field(c, "keyword"),
            art: opt_str_field(c, "art"),
            sort_order: index(n),
            meanings: text_map(&c["meanings"], "cards[].meanings"),
            details: text_map(&c["details"], "cards[].details"),
        })
        .collect();
    let card_art = array(&art["cards"], "card-art.cards")
        .iter()
        .map(|a| {
            let id = str_field(a, "id");
            CardArtRow {
                sort_order: card_order[&id],
                art: str_field(a, "art"),
                width: int_field(a, "width"),
                height: int_field(a, "height"),
                rendered_width: int_field(a, "renderedWidth"),
                rendered_height: int_field(a, "renderedHeight"),
                sha256: str_field(a, "sha256"),
                bytes: int_field(a, "bytes"),
                id,
            }
        })
        .collect();
    let daily_meta = DailyMetaRow {
        schema_version: int_field(&daily, "schema_version"),
        version: str_field(&daily, "version"),
        updated: str_field(&daily, "updated"),
        status: str_field(&daily, "status"),
        scope: str_field(&daily, "scope"),
        auto_fallback_to_general_or_spread: bool_field(
            &daily,
            "auto_fallback_to_general_or_spread",
        ),
    };
    let daily_cards = array(&daily["cards"], "daily.cards")
        .iter()
        .enumerate()
        .map(|(n, c)| {
            let content = &c["content"];
            let variants = &c["variants"];
            DailyCardRow {
                card_id: str_field(c, "card_id"),
                name: str_field(c, "name"),
                arcana: str_field(c, "arcana"),
                reading_mode: str_field(c, "reading_mode"),
                time_horizon: str_field(c, "time_horizon"),
                orientation: str_field(c, "orientation"),
                content_status: str_field(c, "content_status"),
                approved_for_web: bool_field(c, "approved_for_web"),
                theme: str_field(content, "theme"),
                overview: str_field(content, "overview"),
                work: str_field(content, "work"),
                money: str_field(content, "money"),
                rabbit_message: str_field(content, "rabbit_message"),
                colors: str_field(content, "colors"),
                affirmation: str_field(content, "affirmation"),
                summary_theme: opt_str_field(variants, "summary_theme"),
                overview_theme: opt_str_field(variants, "overview_theme"),
                sort_order: index(n),
                love_by_status: text_map(
                    &content["love_by_status"],
                    "daily.cards[].content.love_by_status",
                ),
            }
        })
        .collect();

    MasterDataFixture {
        love_statuses: reversed(love_statuses),
        categories: reversed(categories),
        situations: reversed(situations),
        suggestions: reversed(suggestions),
        positions: reversed(positions),
        cards: reversed(card_rows),
        card_art: reversed(card_art),
        daily_meta: Some(daily_meta),
        daily_cards: reversed(daily_cards),
    }
}

/// Scenario A1 on the memory fixture; mirrors `A1_SQL`.
pub fn apply_a1(fixture: &mut MasterDataFixture) {
    for situation in &mut fixture.situations {
        if situation.category.as_deref() == Some("work") {
            situation.category = None;
        }
    }
    for suggestion in &mut fixture.suggestions {
        if suggestion.category.as_deref() == Some("work") {
            suggestion.category = None;
        }
    }
    fixture
        .suggestions
        .retain(|s| s.category.as_deref() != Some(LOVE_CATEGORY));
    fixture
        .categories
        .retain(|c| c.key != "work" && c.key != LOVE_CATEGORY);
}

pub fn object_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .unwrap_or_else(|| panic!("not an object: {value}"))
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn keys_of(list: &Value, field: &str) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("not an array: {list}"))
        .iter()
        .map(|item| item[field].as_str().unwrap().to_string())
        .collect()
}

pub fn assert_clean(label: &str, text: &str) {
    for marker in FORBIDDEN {
        assert!(!text.contains(marker), "{label} contains `{marker}`");
    }
}

/// Scenario 1.
pub fn assert_cards_match_seed(body: &Value) {
    let expected = seed(CARDS_JSON);
    let expected = expected.as_array().unwrap();
    let actual = body.as_array().expect("cards body is an array");
    assert_eq!(expected.len(), 78, "seed card count");
    assert_eq!(actual.len(), 78, "response card count");
    for (got, want) in actual.iter().zip(expected) {
        assert_eq!(got, want, "card {}", want["id"]);
        assert_eq!(
            object_keys(got),
            ["art", "details", "id", "keyword", "meanings", "name"]
        );
        assert_eq!(keys_of_map(&got["meanings"]), READING_LOVE_ORDER);
    }
}

fn keys_of_map(map: &Value) -> Vec<String> {
    let mut keys: Vec<String> = map.as_object().unwrap().keys().cloned().collect();
    keys.sort_by_key(|k| READING_LOVE_ORDER.iter().position(|o| *o == k.as_str()));
    keys
}

/// Scenario 2 (and the daily half of scenario 4).
pub fn assert_daily_matches_seed(body: &Value) {
    let expected = seed(DAILY_JSON);
    assert_eq!(
        object_keys(body),
        [
            "auto_fallback_to_general_or_spread",
            "cards",
            "love_statuses",
            "schema_version",
            "scope",
            "status",
            "updated",
            "version",
        ]
    );
    for key in [
        "schema_version",
        "version",
        "updated",
        "status",
        "scope",
        "auto_fallback_to_general_or_spread",
        "love_statuses",
    ] {
        assert_eq!(body[key], expected[key], "daily `{key}`");
    }
    assert!(body["auto_fallback_to_general_or_spread"].is_boolean());
    assert_eq!(keys_of(&body["love_statuses"], "key"), DAILY_LOVE_ORDER);

    let actual = body["cards"].as_array().expect("daily cards array");
    let wanted = expected["cards"].as_array().unwrap();
    assert_eq!(actual.len(), 78, "response daily card count");
    assert_eq!(wanted.len(), 78, "seed daily card count");
    let mut null_texts = 0;
    let mut null_complicated = 0;
    for (got, want) in actual.iter().zip(wanted) {
        let mut got_rest = got.clone();
        let computed = got_rest
            .as_object_mut()
            .unwrap()
            .remove("missing_fields")
            .expect("missing_fields present");
        let mut want_rest = want.clone();
        let seed_missing = want_rest.as_object_mut().unwrap().remove("missing_fields");
        assert_eq!(got_rest, want_rest, "daily card {}", want["card_id"]);
        assert!(got["approved_for_web"].is_boolean());

        let love = &want["content"]["love_by_status"];
        let expected_missing: Vec<Value> = DAILY_LOVE_ORDER
            .iter()
            .filter(|key| love[**key].is_null())
            .map(|key| Value::String(format!("content.love_by_status.{key}")))
            .collect();
        assert_eq!(
            computed,
            Value::Array(expected_missing),
            "missing_fields of {}",
            want["card_id"]
        );
        if let Some(seed_missing) = seed_missing {
            assert_eq!(
                computed, seed_missing,
                "seed missing_fields of {}",
                want["card_id"]
            );
        }
        for (key, text) in got["content"]["love_by_status"].as_object().unwrap() {
            if text.is_null() {
                null_texts += 1;
                if key == "complicated" {
                    null_complicated += 1;
                }
            }
        }
    }
    assert_eq!(null_complicated, 22, "null `complicated` daily texts");
    assert_eq!(null_texts, 22, "null daily texts in total");
}

/// Scenario 3 (manifest half).
pub fn assert_card_art_matches_seed(body: &Value) {
    let expected = seed(CARD_ART_JSON);
    assert_eq!(object_keys(body), ["cards"]);
    let actual = body["cards"].as_array().unwrap();
    let wanted = expected["cards"].as_array().unwrap();
    assert_eq!(actual.len(), 74, "response art count");
    assert_eq!(wanted.len(), 74, "seed art count");
    for (got, want) in actual.iter().zip(wanted) {
        assert_eq!(got, want, "art {}", want["id"]);
    }
    let ids = keys_of(&body["cards"], "id");
    for id in NO_ART_CARDS {
        assert!(
            !ids.iter().any(|i| i == id),
            "{id} must not have an art entry"
        );
    }
}

/// Scenario 3 (cards half): the 4 cards without art are `null`, every art entry equals its card's art.
pub fn assert_art_consistent(cards_body: &Value, art_body: &Value) {
    let card_art: HashMap<String, Value> = cards_body
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["id"].as_str().unwrap().to_string(), c["art"].clone()))
        .collect();
    for id in NO_ART_CARDS {
        assert_eq!(card_art[id], Value::Null, "{id} art");
    }
    for entry in art_body["cards"].as_array().unwrap() {
        let id = entry["id"].as_str().unwrap();
        assert_eq!(card_art[id], entry["art"], "art of {id}");
    }
}

fn love_situations(config: &Value) -> Value {
    Value::Array(
        config["love_statuses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| json!({"key": s["key"], "label": s["reading_label"]}))
            .collect(),
    )
}

/// Scenario 4.
pub fn assert_reading_config_matches_seed(body: &Value) {
    let expected = seed(READING_CONFIG_JSON);
    assert_eq!(
        object_keys(body),
        [
            "categories",
            "love_statuses",
            "positions",
            "situations",
            "suggestions",
            "uncategorized_situations",
            "uncategorized_suggestions",
        ]
    );
    assert_eq!(body["categories"], expected["categories"]);
    let mut category_keys = keys_of(&body["categories"], "key");
    category_keys.sort();
    assert_eq!(category_keys, ["life", "love", "money", "work"]);

    assert_eq!(body["love_statuses"], expected["love_statuses"]);
    assert_eq!(keys_of(&body["love_statuses"], "key"), READING_LOVE_ORDER);
    assert_eq!(body["positions"], expected["positions"]);
    assert_eq!(
        keys_of(&body["positions"], "key"),
        ["past", "present", "future"]
    );

    assert_eq!(
        object_keys(&body["situations"]),
        ["life", "love", "money", "work"]
    );
    assert_eq!(body["situations"]["love"], love_situations(&expected));
    for category in ["work", "money", "life"] {
        assert_eq!(
            body["situations"][category], expected["situations"][category],
            "situations.{category}"
        );
        assert_eq!(
            body["situations"][category].as_array().unwrap().len(),
            3,
            "situations.{category}"
        );
    }

    assert_eq!(body["suggestions"], expected["suggestions"]);
    let mut ids: Vec<String> = ["love", "work", "money", "life"]
        .iter()
        .flat_map(|category| keys_of(&body["suggestions"][*category], "id"))
        .collect();
    ids.sort();
    let mut want_ids: Vec<String> = ["love", "work", "money", "life"]
        .iter()
        .flat_map(|category| (1..=3).map(move |n| format!("{category}-{n}")))
        .collect();
    want_ids.sort();
    assert_eq!(ids, want_ids);

    assert_eq!(body["uncategorized_situations"], json!([]));
    assert_eq!(body["uncategorized_suggestions"], json!([]));
}

/// Scenario A1 (after `apply_a1` or `A1_SQL`).
pub fn assert_a1_reading_config(body: &Value) {
    let expected = seed(READING_CONFIG_JSON);
    let remaining: Vec<String> = keys_of(&expected["categories"], "key")
        .into_iter()
        .filter(|k| k != "work" && k != LOVE_CATEGORY)
        .collect();
    assert_eq!(keys_of(&body["categories"], "key"), remaining);
    let mut remaining_sorted = remaining.clone();
    remaining_sorted.sort();
    assert_eq!(object_keys(&body["situations"]), remaining_sorted);
    assert_eq!(object_keys(&body["suggestions"]), remaining_sorted);

    let mut uncategorized = expected["situations"]["work"].as_array().unwrap().clone();
    uncategorized.extend(
        love_situations(&expected)
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    assert_eq!(
        body["uncategorized_situations"],
        Value::Array(uncategorized)
    );
    assert_eq!(keys_of(&body["uncategorized_situations"], "key").len(), 9);
    assert_eq!(
        body["uncategorized_suggestions"],
        expected["suggestions"]["work"]
    );
    assert_eq!(
        keys_of(&body["uncategorized_suggestions"], "id"),
        ["work-1", "work-2", "work-3"]
    );
    assert!(!body.to_string().contains("\"love-1\""));
}
