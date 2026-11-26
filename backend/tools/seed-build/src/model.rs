//! Seed file shapes. Every struct denies unknown fields, so a dropped or misspelled field fails.

use crate::SeedSources;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Love-status key to text; `None` is a JSON `null` (no text stored).
pub type TextByStatus = BTreeMap<String, Option<String>>;

pub struct Seed {
    pub reading_config: ReadingConfig,
    pub cards: Vec<ReadingCard>,
    pub card_art: CardArtManifest,
    pub daily: DailyDocument,
}

impl Seed {
    pub fn parse(sources: &SeedSources) -> Result<Self, String> {
        Ok(Self {
            reading_config: parse("reading-config.json", &sources.reading_config)?,
            cards: parse("cards.json", &sources.cards)?,
            card_art: parse("card-art.json", &sources.card_art)?,
            daily: parse("daily.json", &sources.daily)?,
        })
    }
}

fn parse<T: DeserializeOwned>(name: &str, text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|e| format!("seed/{name}: {e}"))
}

/// `GET /v1/cards` item.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadingCard {
    pub id: String,
    pub name: String,
    pub keyword: String,
    pub art: Option<String>,
    pub meanings: TextByStatus,
    pub details: TextByStatus,
}

/// `GET /v1/daily-cards`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyDocument {
    pub schema_version: i64,
    pub version: String,
    pub updated: String,
    pub status: String,
    pub scope: String,
    pub auto_fallback_to_general_or_spread: bool,
    pub love_statuses: Vec<DailyLoveStatus>,
    pub cards: Vec<DailyCard>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyLoveStatus {
    pub key: String,
    pub source_label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub variants: Variants,
    pub content: DailyContent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variants {
    pub summary_theme: Option<String>,
    pub overview_theme: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyContent {
    pub theme: String,
    pub overview: String,
    pub work: String,
    pub money: String,
    pub love_by_status: TextByStatus,
    pub rabbit_message: String,
    pub colors: String,
    pub affirmation: String,
}

/// `GET /v1/card-art`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardArtManifest {
    pub cards: Vec<CardArt>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

/// `GET /v1/reading-config`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadingConfig {
    pub categories: Vec<Category>,
    pub situations: BTreeMap<String, Vec<LabeledKey>>,
    pub suggestions: BTreeMap<String, Vec<Suggestion>>,
    pub positions: Vec<LabeledKey>,
    pub uncategorized_situations: Vec<LabeledKey>,
    pub uncategorized_suggestions: Vec<Suggestion>,
    pub love_statuses: Vec<LoveStatus>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub hint: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledKey {
    pub key: String,
    pub label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    pub id: String,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoveStatus {
    pub key: String,
    pub daily_label: String,
    pub reading_label: String,
}
