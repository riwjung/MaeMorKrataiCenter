//! Master-data rows (what `MasterDataPort` returns) and the pure assembly of the four
//! public responses defined in `docs/api/client/openapi.yaml` v0.2.1. No I/O here.
//!
//! Both adapters return these rows unsorted. Every ordering rule (SPEC-002 §4.3, D10, D11
//! item 6), the null fill of love-status maps and `missing_fields` (D09) live only here.

use serde::de::{Error as _, Unexpected};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::collections::HashMap;

/// Category key whose situations are derived from love statuses instead of stored rows.
pub const LOVE_CATEGORY: &str = "love";
const MISSING_LOVE_TEXT_PREFIX: &str = "content.love_by_status.";

/// Love-status key to stored text. A key whose child row exists with `NULL` text maps to `None`.
pub type TextMap = HashMap<String, Option<String>>;

// ---- Rows ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoveStatusRow {
    pub key: String,
    pub daily_label: String,
    pub reading_label: String,
    pub daily_sort_order: i64,
    pub reading_sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CardRow {
    pub id: String,
    pub name: String,
    pub keyword: String,
    pub art: Option<String>,
    pub sort_order: i64,
    pub meanings: TextMap,
    pub details: TextMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CardArtRow {
    pub id: String,
    pub art: String,
    pub width: i64,
    pub height: i64,
    pub rendered_width: i64,
    pub rendered_height: i64,
    pub sha256: String,
    pub bytes: i64,
    /// `cards.sort_order` of the card this entry belongs to.
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DailyMetaRow {
    pub schema_version: i64,
    pub version: String,
    pub updated: String,
    pub status: String,
    pub scope: String,
    #[serde(deserialize_with = "sql_bool")]
    pub auto_fallback_to_general_or_spread: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DailyCardRow {
    pub card_id: String,
    pub name: String,
    pub arcana: String,
    pub reading_mode: String,
    pub time_horizon: String,
    pub orientation: String,
    pub content_status: String,
    #[serde(deserialize_with = "sql_bool")]
    pub approved_for_web: bool,
    pub theme: String,
    pub overview: String,
    pub work: String,
    pub money: String,
    pub rabbit_message: String,
    pub colors: String,
    pub affirmation: String,
    pub summary_theme: Option<String>,
    pub overview_theme: Option<String>,
    pub sort_order: i64,
    pub love_by_status: TextMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CategoryRow {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub hint: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SituationRow {
    pub key: String,
    pub category: Option<String>,
    pub label: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SuggestionRow {
    pub id: String,
    pub category: Option<String>,
    pub text: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PositionRow {
    pub key: String,
    pub label: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingCardRows {
    pub love_statuses: Vec<LoveStatusRow>,
    pub cards: Vec<CardRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyRows {
    pub meta: DailyMetaRow,
    pub love_statuses: Vec<LoveStatusRow>,
    pub cards: Vec<DailyCardRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingConfigRows {
    pub categories: Vec<CategoryRow>,
    pub situations: Vec<SituationRow>,
    pub suggestions: Vec<SuggestionRow>,
    pub positions: Vec<PositionRow>,
    pub love_statuses: Vec<LoveStatusRow>,
}

/// D1 (SQLite) stores booleans as INTEGER 0/1, and `json_object` keeps them as numbers.
pub fn sql_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    match i64::deserialize(deserializer)? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(D::Error::invalid_value(
            Unexpected::Signed(other),
            &"0 or 1",
        )),
    }
}

// ---- Responses (openapi.yaml v0.2.1) -------------------------------------------------

/// JSON object that keeps insertion order (category order, love-status order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedMap<V>(pub Vec<(String, V)>);

impl<V: Serialize> Serialize for OrderedMap<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadingCard {
    pub id: String,
    pub name: String,
    pub keyword: String,
    pub art: Option<String>,
    pub meanings: OrderedMap<Option<String>>,
    pub details: OrderedMap<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyCardsDocument {
    pub schema_version: i64,
    pub version: String,
    pub updated: String,
    pub status: String,
    pub scope: String,
    pub auto_fallback_to_general_or_spread: bool,
    pub love_statuses: Vec<DailyLoveStatus>,
    pub cards: Vec<DailyCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyLoveStatus {
    pub key: String,
    pub source_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyCard {
    pub card_id: String,
    pub name: String,
    pub arcana: String,
    pub reading_mode: String,
    pub time_horizon: String,
    pub orientation: String,
    pub content_status: String,
    pub approved_for_web: bool,
    pub missing_fields: Vec<String>,
    pub variants: DailyVariants,
    pub content: DailyCardContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyVariants {
    pub summary_theme: Option<String>,
    pub overview_theme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyCardContent {
    pub theme: String,
    pub overview: String,
    pub work: String,
    pub money: String,
    pub love_by_status: OrderedMap<Option<String>>,
    pub rabbit_message: String,
    pub colors: String,
    pub affirmation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardArtManifest {
    pub cards: Vec<CardArt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardArt {
    pub id: String,
    pub art: String,
    pub width: i64,
    pub height: i64,
    #[serde(rename = "renderedWidth")]
    pub rendered_width: i64,
    #[serde(rename = "renderedHeight")]
    pub rendered_height: i64,
    pub sha256: String,
    pub bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadingConfig {
    pub categories: Vec<Category>,
    pub situations: OrderedMap<Vec<LabeledKey>>,
    pub suggestions: OrderedMap<Vec<Suggestion>>,
    pub positions: Vec<LabeledKey>,
    pub love_statuses: Vec<LoveStatus>,
    pub uncategorized_situations: Vec<LabeledKey>,
    pub uncategorized_suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Category {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LabeledKey {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoveStatus {
    pub key: String,
    pub daily_label: String,
    pub reading_label: String,
}

// ---- Assembly -----------------------------------------------------------------------

fn by_order_then_key(a_order: i64, a_key: &str, b_order: i64, b_key: &str) -> Ordering {
    a_order.cmp(&b_order).then_with(|| a_key.cmp(b_key))
}

fn in_reading_order(mut statuses: Vec<LoveStatusRow>) -> Vec<LoveStatusRow> {
    statuses.sort_by(|a, b| {
        by_order_then_key(a.reading_sort_order, &a.key, b.reading_sort_order, &b.key)
    });
    statuses
}

fn in_daily_order(mut statuses: Vec<LoveStatusRow>) -> Vec<LoveStatusRow> {
    statuses
        .sort_by(|a, b| by_order_then_key(a.daily_sort_order, &a.key, b.daily_sort_order, &b.key));
    statuses
}

/// One entry per current love-status key, in `keys` order; `null` where no text is stored.
/// Stored keys that are not current love statuses are dropped.
fn fill_text_map(keys: &[String], mut texts: TextMap) -> OrderedMap<Option<String>> {
    OrderedMap(
        keys.iter()
            .map(|key| (key.clone(), texts.remove(key).flatten()))
            .collect(),
    )
}

pub fn assemble_reading_cards(rows: ReadingCardRows) -> Vec<ReadingCard> {
    let keys: Vec<String> = in_reading_order(rows.love_statuses)
        .into_iter()
        .map(|status| status.key)
        .collect();
    let mut cards = rows.cards;
    cards.sort_by(|a, b| by_order_then_key(a.sort_order, &a.id, b.sort_order, &b.id));
    cards
        .into_iter()
        .map(|card| ReadingCard {
            meanings: fill_text_map(&keys, card.meanings),
            details: fill_text_map(&keys, card.details),
            id: card.id,
            name: card.name,
            keyword: card.keyword,
            art: card.art,
        })
        .collect()
}

fn daily_card(keys: &[String], card: DailyCardRow) -> DailyCard {
    let love_by_status = fill_text_map(keys, card.love_by_status);
    let missing_fields = love_by_status
        .0
        .iter()
        .filter(|(_, text)| text.is_none())
        .map(|(key, _)| format!("{MISSING_LOVE_TEXT_PREFIX}{key}"))
        .collect();
    DailyCard {
        card_id: card.card_id,
        name: card.name,
        arcana: card.arcana,
        reading_mode: card.reading_mode,
        time_horizon: card.time_horizon,
        orientation: card.orientation,
        content_status: card.content_status,
        approved_for_web: card.approved_for_web,
        missing_fields,
        variants: DailyVariants {
            summary_theme: card.summary_theme,
            overview_theme: card.overview_theme,
        },
        content: DailyCardContent {
            theme: card.theme,
            overview: card.overview,
            work: card.work,
            money: card.money,
            love_by_status,
            rabbit_message: card.rabbit_message,
            colors: card.colors,
            affirmation: card.affirmation,
        },
    }
}

pub fn assemble_daily_cards(rows: DailyRows) -> DailyCardsDocument {
    let statuses = in_daily_order(rows.love_statuses);
    let keys: Vec<String> = statuses.iter().map(|status| status.key.clone()).collect();
    let love_statuses = statuses
        .into_iter()
        .map(|status| DailyLoveStatus {
            key: status.key,
            source_label: status.daily_label,
        })
        .collect();
    let mut cards = rows.cards;
    cards.sort_by(|a, b| by_order_then_key(a.sort_order, &a.card_id, b.sort_order, &b.card_id));
    let cards = cards
        .into_iter()
        .map(|card| daily_card(&keys, card))
        .collect();
    let meta = rows.meta;
    DailyCardsDocument {
        schema_version: meta.schema_version,
        version: meta.version,
        updated: meta.updated,
        status: meta.status,
        scope: meta.scope,
        auto_fallback_to_general_or_spread: meta.auto_fallback_to_general_or_spread,
        love_statuses,
        cards,
    }
}

pub fn assemble_card_art(rows: Vec<CardArtRow>) -> CardArtManifest {
    let mut rows = rows;
    rows.sort_by(|a, b| by_order_then_key(a.sort_order, &a.id, b.sort_order, &b.id));
    CardArtManifest {
        cards: rows
            .into_iter()
            .map(|row| CardArt {
                id: row.id,
                art: row.art,
                width: row.width,
                height: row.height,
                rendered_width: row.rendered_width,
                rendered_height: row.rendered_height,
                sha256: row.sha256,
                bytes: row.bytes,
            })
            .collect(),
    }
}

fn labeled(situation: &SituationRow) -> LabeledKey {
    LabeledKey {
        key: situation.key.clone(),
        label: situation.label.clone(),
    }
}

fn suggestion(row: &SuggestionRow) -> Suggestion {
    Suggestion {
        id: row.id.clone(),
        text: row.text.clone(),
    }
}

pub fn assemble_reading_config(rows: ReadingConfigRows) -> ReadingConfig {
    let mut categories = rows.categories;
    categories.sort_by(|a, b| by_order_then_key(a.sort_order, &a.key, b.sort_order, &b.key));
    let mut situations = rows.situations;
    situations.sort_by(|a, b| by_order_then_key(a.sort_order, &a.key, b.sort_order, &b.key));
    let mut suggestions = rows.suggestions;
    suggestions.sort_by(|a, b| by_order_then_key(a.sort_order, &a.id, b.sort_order, &b.id));
    let mut positions = rows.positions;
    positions.sort_by(|a, b| by_order_then_key(a.sort_order, &a.key, b.sort_order, &b.key));
    let love_statuses = in_reading_order(rows.love_statuses);

    let love_situations: Vec<LabeledKey> = love_statuses
        .iter()
        .map(|status| LabeledKey {
            key: status.key.clone(),
            label: status.reading_label.clone(),
        })
        .collect();
    let has_love_category = categories.iter().any(|c| c.key == LOVE_CATEGORY);

    let situations_by_category = OrderedMap(
        categories
            .iter()
            .map(|category| {
                let list = if category.key == LOVE_CATEGORY {
                    love_situations.clone()
                } else {
                    situations
                        .iter()
                        .filter(|s| s.category.as_deref() == Some(category.key.as_str()))
                        .map(labeled)
                        .collect()
                };
                (category.key.clone(), list)
            })
            .collect(),
    );
    let suggestions_by_category = OrderedMap(
        categories
            .iter()
            .map(|category| {
                let list = suggestions
                    .iter()
                    .filter(|s| s.category.as_deref() == Some(category.key.as_str()))
                    .map(suggestion)
                    .collect();
                (category.key.clone(), list)
            })
            .collect(),
    );

    let mut uncategorized_situations: Vec<LabeledKey> = situations
        .iter()
        .filter(|s| s.category.is_none())
        .map(labeled)
        .collect();
    if !has_love_category {
        uncategorized_situations.extend(love_situations);
    }
    let uncategorized_suggestions = suggestions
        .iter()
        .filter(|s| s.category.is_none())
        .map(suggestion)
        .collect();

    ReadingConfig {
        categories: categories
            .into_iter()
            .map(|c| Category {
                key: c.key,
                label: c.label,
                icon: c.icon,
                hint: c.hint,
            })
            .collect(),
        situations: situations_by_category,
        suggestions: suggestions_by_category,
        positions: positions
            .into_iter()
            .map(|p| LabeledKey {
                key: p.key,
                label: p.label,
            })
            .collect(),
        love_statuses: love_statuses
            .into_iter()
            .map(|s| LoveStatus {
                key: s.key,
                daily_label: s.daily_label,
                reading_label: s.reading_label,
            })
            .collect(),
        uncategorized_situations,
        uncategorized_suggestions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn status(key: &str, daily: i64, reading: i64) -> LoveStatusRow {
        LoveStatusRow {
            key: key.to_string(),
            daily_label: format!("daily-{key}"),
            reading_label: format!("reading-{key}"),
            daily_sort_order: daily,
            reading_sort_order: reading,
        }
    }

    /// single: daily 0 / reading 0; crush: daily 1 / reading 2; complicated: daily 2 / reading 1.
    fn statuses() -> Vec<LoveStatusRow> {
        vec![
            status("complicated", 2, 1),
            status("single", 0, 0),
            status("crush", 1, 2),
        ]
    }

    fn texts(pairs: &[(&str, Option<&str>)]) -> TextMap {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
            .collect()
    }

    fn card(id: &str, sort_order: i64) -> CardRow {
        CardRow {
            id: id.to_string(),
            name: format!("name-{id}"),
            keyword: format!("keyword-{id}"),
            art: None,
            sort_order,
            meanings: TextMap::new(),
            details: TextMap::new(),
        }
    }

    fn daily_row(card_id: &str, sort_order: i64, love: TextMap) -> DailyCardRow {
        DailyCardRow {
            card_id: card_id.to_string(),
            name: format!("name-{card_id}"),
            arcana: "major".to_string(),
            reading_mode: "daily_single".to_string(),
            time_horizon: "today".to_string(),
            orientation: "upright_only".to_string(),
            content_status: "assistant_draft_pending_review".to_string(),
            approved_for_web: false,
            theme: "theme".to_string(),
            overview: "overview".to_string(),
            work: "work".to_string(),
            money: "money".to_string(),
            rabbit_message: "rabbit".to_string(),
            colors: "colors".to_string(),
            affirmation: "affirmation".to_string(),
            summary_theme: Some("summary".to_string()),
            overview_theme: None,
            sort_order,
            love_by_status: love,
        }
    }

    fn meta() -> DailyMetaRow {
        DailyMetaRow {
            schema_version: 1,
            version: "daily-v1-draft-2026-11-21".to_string(),
            updated: "2026-11-21".to_string(),
            status: "draft_pending_owner_review".to_string(),
            scope: "daily".to_string(),
            auto_fallback_to_general_or_spread: true,
        }
    }

    fn object_keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    fn sorted(keys: &[&str]) -> Vec<String> {
        let mut keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
        keys.sort();
        keys
    }

    #[test]
    fn reading_cards_fill_every_love_status_in_reading_order() {
        let mut fool = card("the-fool", 0);
        fool.art = Some("/cards/v3/the-fool-e11e1ee168a4.webp".to_string());
        fool.meanings = texts(&[
            ("single", Some("m-single")),
            ("widowed", Some("not a status")),
        ]);
        fool.details = texts(&[("crush", None), ("complicated", Some("d-complicated"))]);
        let cards = assemble_reading_cards(ReadingCardRows {
            love_statuses: statuses(),
            cards: vec![fool],
        });

        let meaning_keys: Vec<&str> = cards[0]
            .meanings
            .0
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(meaning_keys, ["single", "complicated", "crush"]);
        assert_eq!(
            cards[0].meanings.0,
            vec![
                ("single".to_string(), Some("m-single".to_string())),
                ("complicated".to_string(), None),
                ("crush".to_string(), None),
            ]
        );
        assert_eq!(
            cards[0].details.0,
            vec![
                ("single".to_string(), None),
                ("complicated".to_string(), Some("d-complicated".to_string())),
                ("crush".to_string(), None),
            ]
        );
        let body = serde_json::to_string(&cards[0]).unwrap();
        assert!(!body.contains("widowed"));
        assert!(body.find("\"single\"").unwrap() < body.find("\"crush\"").unwrap());
    }

    #[test]
    fn reading_cards_sort_by_sort_order_then_id() {
        let cards = assemble_reading_cards(ReadingCardRows {
            love_statuses: statuses(),
            cards: vec![card("b-card", 1), card("a-card", 1), card("c-card", 0)],
        });
        let ids: Vec<&str> = cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["c-card", "a-card", "b-card"]);
    }

    #[test]
    fn daily_missing_fields_follow_daily_order_for_null_and_absent_texts() {
        let love = texts(&[("single", Some("s")), ("complicated", None)]);
        let doc = assemble_daily_cards(DailyRows {
            meta: meta(),
            love_statuses: statuses(),
            cards: vec![daily_row("the-fool", 0, love)],
        });

        let status_keys: Vec<&str> = doc.love_statuses.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(status_keys, ["single", "crush", "complicated"]);
        assert_eq!(doc.love_statuses[0].source_label, "daily-single");
        assert_eq!(
            doc.cards[0].missing_fields,
            vec![
                "content.love_by_status.crush".to_string(),
                "content.love_by_status.complicated".to_string(),
            ]
        );
        assert_eq!(
            doc.cards[0].content.love_by_status.0,
            vec![
                ("single".to_string(), Some("s".to_string())),
                ("crush".to_string(), None),
                ("complicated".to_string(), None),
            ]
        );
        assert!(doc.auto_fallback_to_general_or_spread);
        assert!(!doc.cards[0].approved_for_web);
        assert_eq!(doc.cards[0].variants.overview_theme, None);
    }

    #[test]
    fn daily_cards_sort_by_sort_order_then_card_id_and_full_texts_have_no_missing_fields() {
        let full = texts(&[
            ("single", Some("a")),
            ("crush", Some("b")),
            ("complicated", Some("c")),
        ]);
        let doc = assemble_daily_cards(DailyRows {
            meta: meta(),
            love_statuses: statuses(),
            cards: vec![
                daily_row("z-card", 0, full.clone()),
                daily_row("b-card", 1, full.clone()),
                daily_row("a-card", 1, full),
            ],
        });
        let ids: Vec<&str> = doc.cards.iter().map(|c| c.card_id.as_str()).collect();
        assert_eq!(ids, ["z-card", "a-card", "b-card"]);
        assert!(doc.cards.iter().all(|c| c.missing_fields.is_empty()));
    }

    #[test]
    fn daily_document_json_has_exactly_the_contract_keys() {
        let doc = assemble_daily_cards(DailyRows {
            meta: meta(),
            love_statuses: statuses(),
            cards: vec![daily_row("the-fool", 0, TextMap::new())],
        });
        let value = serde_json::to_value(&doc).unwrap();
        assert_eq!(
            object_keys(&value),
            sorted(&[
                "schema_version",
                "version",
                "updated",
                "status",
                "scope",
                "auto_fallback_to_general_or_spread",
                "love_statuses",
                "cards",
            ])
        );
        let card = &value["cards"][0];
        assert_eq!(
            object_keys(card),
            sorted(&[
                "card_id",
                "name",
                "arcana",
                "reading_mode",
                "time_horizon",
                "orientation",
                "content_status",
                "approved_for_web",
                "missing_fields",
                "variants",
                "content",
            ])
        );
        assert_eq!(
            object_keys(&card["content"]),
            sorted(&[
                "theme",
                "overview",
                "work",
                "money",
                "love_by_status",
                "rabbit_message",
                "colors",
                "affirmation",
            ])
        );
        assert_eq!(card["approved_for_web"], json!(false));
        assert_eq!(value["auto_fallback_to_general_or_spread"], json!(true));
    }

    #[test]
    fn card_art_sorts_by_card_order_and_uses_contract_key_names() {
        let art = |id: &str, sort_order: i64| CardArtRow {
            id: id.to_string(),
            art: format!("/cards/v3/{id}-0123456789ab.webp"),
            width: 637,
            height: 1024,
            rendered_width: 600,
            rendered_height: 965,
            sha256: "a".repeat(64),
            bytes: 1000,
            sort_order,
        };
        let manifest = assemble_card_art(vec![art("b-card", 2), art("a-card", 1)]);
        let value = serde_json::to_value(&manifest).unwrap();
        assert_eq!(value["cards"][0]["id"], "a-card");
        assert_eq!(value["cards"][1]["id"], "b-card");
        assert_eq!(
            object_keys(&value["cards"][0]),
            sorted(&[
                "id",
                "art",
                "width",
                "height",
                "renderedWidth",
                "renderedHeight",
                "sha256",
                "bytes"
            ])
        );
        assert_eq!(value["cards"][0]["renderedWidth"], 600);
    }

    fn category(key: &str, sort_order: i64) -> CategoryRow {
        CategoryRow {
            key: key.to_string(),
            label: format!("label-{key}"),
            icon: "*".to_string(),
            hint: format!("hint-{key}"),
            sort_order,
        }
    }

    fn situation(key: &str, category: Option<&str>, sort_order: i64) -> SituationRow {
        SituationRow {
            key: key.to_string(),
            category: category.map(str::to_string),
            label: format!("label-{key}"),
            sort_order,
        }
    }

    fn suggestion_row(id: &str, category: Option<&str>, sort_order: i64) -> SuggestionRow {
        SuggestionRow {
            id: id.to_string(),
            category: category.map(str::to_string),
            text: format!("text-{id}"),
            sort_order,
        }
    }

    fn positions() -> Vec<PositionRow> {
        vec![
            PositionRow {
                key: "future".to_string(),
                label: "อนาคต".to_string(),
                sort_order: 2,
            },
            PositionRow {
                key: "past".to_string(),
                label: "อดีต".to_string(),
                sort_order: 0,
            },
            PositionRow {
                key: "present".to_string(),
                label: "ปัจจุบัน".to_string(),
                sort_order: 1,
            },
        ]
    }

    #[test]
    fn reading_config_groups_by_category_order_and_derives_love_situations() {
        let config = assemble_reading_config(ReadingConfigRows {
            categories: vec![category("work", 1), category("love", 0)],
            situations: vec![
                situation("b_key", Some("work"), 0),
                situation("a_key", Some("work"), 0),
                situation("c_key", Some("work"), -1),
            ],
            suggestions: vec![
                suggestion_row("work-2", Some("work"), 1),
                suggestion_row("work-1", Some("work"), 0),
                suggestion_row("love-1", Some("love"), 0),
            ],
            positions: positions(),
            love_statuses: statuses(),
        });

        let category_keys: Vec<&str> = config.categories.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(category_keys, ["love", "work"]);
        let situation_groups: Vec<&str> = config
            .situations
            .0
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(situation_groups, ["love", "work"]);
        let love: Vec<(&str, &str)> = config.situations.0[0]
            .1
            .iter()
            .map(|s| (s.key.as_str(), s.label.as_str()))
            .collect();
        assert_eq!(
            love,
            [
                ("single", "reading-single"),
                ("complicated", "reading-complicated"),
                ("crush", "reading-crush")
            ]
        );
        let work: Vec<&str> = config.situations.0[1]
            .1
            .iter()
            .map(|s| s.key.as_str())
            .collect();
        assert_eq!(work, ["c_key", "a_key", "b_key"]);
        let work_suggestions: Vec<&str> = config.suggestions.0[1]
            .1
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(work_suggestions, ["work-1", "work-2"]);
        let position_keys: Vec<&str> = config.positions.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(position_keys, ["past", "present", "future"]);
        let status_keys: Vec<&str> = config
            .love_statuses
            .iter()
            .map(|s| s.key.as_str())
            .collect();
        assert_eq!(status_keys, ["single", "complicated", "crush"]);
        assert!(config.uncategorized_situations.is_empty());
        assert!(config.uncategorized_suggestions.is_empty());
    }

    #[test]
    fn reading_config_lists_uncategorized_then_love_derived_when_no_love_category() {
        let config = assemble_reading_config(ReadingConfigRows {
            categories: vec![category("money", 2)],
            situations: vec![
                situation("own_business", None, 2),
                situation("job_seeking", None, 1),
                situation("current_job", None, 1),
                situation("saving_plan", Some("money"), 0),
            ],
            suggestions: vec![
                suggestion_row("work-3", None, 2),
                suggestion_row("work-2", None, 0),
                suggestion_row("work-1", None, 0),
                suggestion_row("money-1", Some("money"), 0),
            ],
            positions: positions(),
            love_statuses: statuses(),
        });

        let groups: Vec<&str> = config
            .situations
            .0
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(groups, ["money"]);
        let uncategorized: Vec<&str> = config
            .uncategorized_situations
            .iter()
            .map(|s| s.key.as_str())
            .collect();
        assert_eq!(
            uncategorized,
            [
                "current_job",
                "job_seeking",
                "own_business",
                "single",
                "complicated",
                "crush"
            ]
        );
        assert_eq!(config.uncategorized_situations[3].label, "reading-single");
        let uncategorized_ids: Vec<&str> = config
            .uncategorized_suggestions
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(uncategorized_ids, ["work-1", "work-2", "work-3"]);
        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(
            object_keys(&value),
            sorted(&[
                "categories",
                "situations",
                "suggestions",
                "positions",
                "love_statuses",
                "uncategorized_situations",
                "uncategorized_suggestions",
            ])
        );
    }

    #[test]
    fn sql_bool_accepts_only_zero_and_one() {
        let parse = |flag: &str| {
            serde_json::from_str::<DailyMetaRow>(&format!(
                r#"{{"schema_version":1,"version":"v","updated":"2026-11-21","status":"s","scope":"x","auto_fallback_to_general_or_spread":{flag}}}"#
            ))
        };
        assert!(parse("1").unwrap().auto_fallback_to_general_or_spread);
        assert!(!parse("0").unwrap().auto_fallback_to_general_or_spread);
        assert!(parse("2").is_err());
        assert!(parse("true").is_err());
        assert!(parse("null").is_err());
    }

    #[test]
    fn reading_card_json_has_exactly_the_contract_keys_and_null_art() {
        let cards = assemble_reading_cards(ReadingCardRows {
            love_statuses: statuses(),
            cards: vec![card("six-of-wands", 0)],
        });
        let value = serde_json::to_value(&cards[0]).unwrap();
        assert_eq!(
            object_keys(&value),
            sorted(&["id", "name", "keyword", "art", "meanings", "details"])
        );
        assert_eq!(value["art"], Value::Null);
        assert_eq!(value["meanings"]["single"], Value::Null);
    }
}
