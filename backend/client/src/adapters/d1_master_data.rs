//! D1 side of the master-data port (battleground B1). Every statement is a static
//! `SELECT` returning one TEXT column `row_json` built with `json_object`, so one string
//! per row crosses the JS/wasm boundary; serde_json parses it into the typed rows of
//! `crate::master_data`. Nothing in this module writes to D1 (SPEC-002a §6).

use crate::error::ClientError;
use crate::master_data::{
    CardArtRow, CardRow, CategoryRow, DailyCardRow, DailyMetaRow, DailyRows, LoveStatusRow,
    PositionRow, ReadingCardRows, ReadingConfigRows, SituationRow, SuggestionRow,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use worker::send::SendFuture;
use worker::D1Database;

pub const LOVE_STATUSES_SQL: &str = r#"SELECT json_object('key', "key", 'daily_label', daily_label, 'reading_label', reading_label, 'daily_sort_order', daily_sort_order, 'reading_sort_order', reading_sort_order) AS row_json FROM love_statuses"#;

pub const CARDS_SQL: &str = r#"SELECT json_object('id', c.id, 'name', c.name, 'keyword', c.keyword, 'art', c.art, 'sort_order', c.sort_order, 'meanings', json((SELECT json_group_object(m.love_status, m.text) FROM card_meanings m WHERE m.card_id = c.id)), 'details', json((SELECT json_group_object(d.love_status, d.text) FROM card_details d WHERE d.card_id = c.id))) AS row_json FROM cards c"#;

pub const CARD_ART_SQL: &str = r#"SELECT json_object('id', a.card_id, 'art', a.art, 'width', a.width, 'height', a.height, 'rendered_width', a.rendered_width, 'rendered_height', a.rendered_height, 'sha256', a.sha256, 'bytes', a.bytes, 'sort_order', c.sort_order) AS row_json FROM card_art a JOIN cards c ON c.id = a.card_id"#;

pub const DAILY_META_SQL: &str = r#"SELECT json_object('schema_version', schema_version, 'version', version, 'updated', updated, 'status', status, 'scope', scope, 'auto_fallback_to_general_or_spread', auto_fallback_to_general_or_spread) AS row_json FROM daily_meta WHERE id = 1"#;

pub const DAILY_CARDS_SQL: &str = r#"SELECT json_object('card_id', d.card_id, 'name', d.name, 'arcana', d.arcana, 'reading_mode', d.reading_mode, 'time_horizon', d.time_horizon, 'orientation', d.orientation, 'content_status', d.content_status, 'approved_for_web', d.approved_for_web, 'theme', d.theme, 'overview', d.overview, 'work', d.work, 'money', d.money, 'rabbit_message', d.rabbit_message, 'colors', d.colors, 'affirmation', d.affirmation, 'summary_theme', d.summary_theme, 'overview_theme', d.overview_theme, 'sort_order', d.sort_order, 'love_by_status', json((SELECT json_group_object(t.love_status, t.text) FROM daily_love_texts t WHERE t.card_id = d.card_id))) AS row_json FROM daily_cards d"#;

pub const CATEGORIES_SQL: &str = r#"SELECT json_object('key', "key", 'label', label, 'icon', icon, 'hint', hint, 'sort_order', sort_order) AS row_json FROM categories"#;

pub const SITUATIONS_SQL: &str = r#"SELECT json_object('key', "key", 'category', category, 'label', label, 'sort_order', sort_order) AS row_json FROM situations"#;

pub const SUGGESTIONS_SQL: &str = r#"SELECT json_object('id', id, 'category', category, 'text', text, 'sort_order', sort_order) AS row_json FROM suggestions"#;

pub const POSITIONS_SQL: &str = r#"SELECT json_object('key', "key", 'label', label, 'sort_order', sort_order) AS row_json FROM positions"#;

/// One `D1Database::batch` per endpoint. The `*_from_sets` functions read the result sets
/// in this order.
pub const READING_CARDS_BATCH: &[&str] = &[LOVE_STATUSES_SQL, CARDS_SQL];
pub const DAILY_CARDS_BATCH: &[&str] = &[DAILY_META_SQL, LOVE_STATUSES_SQL, DAILY_CARDS_SQL];
pub const CARD_ART_BATCH: &[&str] = &[CARD_ART_SQL];
pub const READING_CONFIG_BATCH: &[&str] = &[
    CATEGORIES_SQL,
    SITUATIONS_SQL,
    SUGGESTIONS_SQL,
    POSITIONS_SQL,
    LOVE_STATUSES_SQL,
];

#[derive(Deserialize)]
struct SqlRow {
    row_json: String,
}

/// Runs `statements` as one D1 batch and returns the `row_json` strings of each statement.
pub async fn query_row_sets(
    db: D1Database,
    statements: &'static [&'static str],
) -> Result<Vec<Vec<String>>, ClientError> {
    SendFuture::new(async move {
        let prepared = statements.iter().map(|&sql| db.prepare(sql)).collect();
        let results = db
            .batch(prepared)
            .await
            .map_err(|e| ClientError::service_unavailable(format!("D1 batch error: {e}")))?;
        results
            .iter()
            .map(|result| {
                result
                    .results::<SqlRow>()
                    .map(|rows| rows.into_iter().map(|row| row.row_json).collect())
                    .map_err(|e| ClientError::internal(format!("D1 result decode error: {e}")))
            })
            .collect()
    })
    .await
}

pub fn parse_rows<T: DeserializeOwned>(rows: &[String]) -> Result<Vec<T>, ClientError> {
    rows.iter()
        .map(|row| {
            serde_json::from_str(row)
                .map_err(|e| ClientError::internal(format!("master-data row decode error: {e}")))
        })
        .collect()
}

fn exact_sets<const N: usize>(sets: Vec<Vec<String>>) -> Result<[Vec<String>; N], ClientError> {
    let count = sets.len();
    sets.try_into()
        .map_err(|_| ClientError::internal(format!("expected {N} D1 result sets, got {count}")))
}

pub fn reading_cards_from_sets(sets: Vec<Vec<String>>) -> Result<ReadingCardRows, ClientError> {
    let [love_statuses, cards] = exact_sets::<2>(sets)?;
    Ok(ReadingCardRows {
        love_statuses: parse_rows::<LoveStatusRow>(&love_statuses)?,
        cards: parse_rows::<CardRow>(&cards)?,
    })
}

pub fn daily_from_sets(sets: Vec<Vec<String>>) -> Result<DailyRows, ClientError> {
    let [meta, love_statuses, cards] = exact_sets::<3>(sets)?;
    let meta = parse_rows::<DailyMetaRow>(&meta)?
        .into_iter()
        .next()
        .ok_or_else(|| ClientError::service_unavailable("daily_meta row missing; run the seed"))?;
    Ok(DailyRows {
        meta,
        love_statuses: parse_rows::<LoveStatusRow>(&love_statuses)?,
        cards: parse_rows::<DailyCardRow>(&cards)?,
    })
}

pub fn card_art_from_sets(sets: Vec<Vec<String>>) -> Result<Vec<CardArtRow>, ClientError> {
    let [art] = exact_sets::<1>(sets)?;
    parse_rows::<CardArtRow>(&art)
}

pub fn reading_config_from_sets(sets: Vec<Vec<String>>) -> Result<ReadingConfigRows, ClientError> {
    let [categories, situations, suggestions, positions, love_statuses] = exact_sets::<5>(sets)?;
    Ok(ReadingConfigRows {
        categories: parse_rows::<CategoryRow>(&categories)?,
        situations: parse_rows::<SituationRow>(&situations)?,
        suggestions: parse_rows::<SuggestionRow>(&suggestions)?,
        positions: parse_rows::<PositionRow>(&positions)?,
        love_statuses: parse_rows::<LoveStatusRow>(&love_statuses)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    fn strings(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|r| r.to_string()).collect()
    }

    const STATUS_ROW: &str = r#"{"key":"single","daily_label":"คนโสด","reading_label":"คนโสด","daily_sort_order":0,"reading_sort_order":0}"#;

    fn daily_card_json(approved: &str) -> String {
        format!(
            r#"{{"card_id":"the-fool","name":"The Fool","arcana":"major","reading_mode":"daily_single","time_horizon":"today","orientation":"upright_only","content_status":"assistant_draft_pending_review","approved_for_web":{approved},"theme":"t","overview":"o","work":"w","money":"m","rabbit_message":"r","colors":"c","affirmation":"a","summary_theme":"s","overview_theme":null,"sort_order":0,"love_by_status":{{"single":"x","complicated":null}}}}"#
        )
    }

    const META_ROW: &str = r#"{"schema_version":1,"version":"daily-v1-draft-2026-11-21","updated":"2026-11-21","status":"draft_pending_owner_review","scope":"daily","auto_fallback_to_general_or_spread":1}"#;

    #[test]
    fn parses_card_rows_as_sqlite_json_object_writes_them() {
        let rows = reading_cards_from_sets(vec![
            strings(&[STATUS_ROW]),
            strings(&[
                r#"{"id":"the-fool","name":"The Fool","keyword":"การเริ่มต้น","art":"/cards/v3/the-fool-e11e1ee168a4.webp","sort_order":0,"meanings":{"single":"m","complicated":null},"details":{}}"#,
                r#"{"id":"six-of-wands","name":"Six of Wands","keyword":"k","art":null,"sort_order":1,"meanings":{},"details":{}}"#,
            ]),
        ])
        .unwrap();
        assert_eq!(rows.love_statuses[0].key, "single");
        assert_eq!(rows.cards.len(), 2);
        assert_eq!(
            rows.cards[0].meanings.get("single"),
            Some(&Some("m".to_string()))
        );
        assert_eq!(rows.cards[0].meanings.get("complicated"), Some(&None));
        assert!(rows.cards[0].details.is_empty());
        assert_eq!(rows.cards[1].art, None);
    }

    #[test]
    fn daily_rows_map_integer_flags_to_bool() {
        let rows = daily_from_sets(vec![
            strings(&[META_ROW]),
            strings(&[STATUS_ROW]),
            vec![daily_card_json("0")],
        ])
        .unwrap();
        assert!(rows.meta.auto_fallback_to_general_or_spread);
        assert!(!rows.cards[0].approved_for_web);
        assert_eq!(rows.cards[0].overview_theme, None);
        assert_eq!(rows.cards[0].love_by_status.get("complicated"), Some(&None));

        let approved = daily_from_sets(vec![
            strings(&[META_ROW]),
            strings(&[STATUS_ROW]),
            vec![daily_card_json("1")],
        ])
        .unwrap();
        assert!(approved.cards[0].approved_for_web);
    }

    #[test]
    fn out_of_range_flag_is_an_internal_error() {
        let err = daily_from_sets(vec![
            strings(&[META_ROW]),
            strings(&[STATUS_ROW]),
            vec![daily_card_json("2")],
        ])
        .unwrap_err();
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.code, "INTERNAL_ERROR");
    }

    #[test]
    fn missing_daily_meta_is_service_unavailable() {
        let err = daily_from_sets(vec![vec![], strings(&[STATUS_ROW]), vec![]]).unwrap_err();
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn wrong_result_set_count_is_an_internal_error() {
        let err = reading_cards_from_sets(vec![vec![]]).unwrap_err();
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        let err = card_art_from_sets(vec![]).unwrap_err();
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn reading_config_rows_accept_null_category() {
        let rows = reading_config_from_sets(vec![
            strings(&[r#"{"key":"love","label":"ความรัก","icon":"♡","hint":"h","sort_order":0}"#]),
            strings(&[
                r#"{"key":"current_job","category":null,"label":"งานปัจจุบัน","sort_order":0}"#,
            ]),
            strings(&[r#"{"id":"work-1","category":null,"text":"q","sort_order":0}"#]),
            strings(&[r#"{"key":"past","label":"อดีต","sort_order":0}"#]),
            strings(&[STATUS_ROW]),
        ])
        .unwrap();
        assert_eq!(rows.situations[0].category, None);
        assert_eq!(rows.suggestions[0].category, None);
        assert_eq!(rows.categories[0].key, "love");
        assert_eq!(
            card_art_from_sets(vec![strings(&[
                r#"{"id":"the-fool","art":"/cards/v3/the-fool-e11e1ee168a4.webp","width":637,"height":1024,"rendered_width":600,"rendered_height":965,"sha256":"aa","bytes":10,"sort_order":0}"#
            ])])
            .unwrap()[0]
                .rendered_height,
            965
        );
    }

    #[test]
    fn every_statement_is_a_single_read_only_select() {
        const WRITE_WORDS: [&str; 10] = [
            "INSERT", "UPDATE", "DELETE", "REPLACE", "UPSERT", "DROP", "ALTER", "CREATE", "PRAGMA",
            "ATTACH",
        ];
        let all = READING_CARDS_BATCH
            .iter()
            .chain(DAILY_CARDS_BATCH)
            .chain(CARD_ART_BATCH)
            .chain(READING_CONFIG_BATCH);
        for sql in all {
            assert!(sql.starts_with("SELECT json_object("), "{sql}");
            assert!(sql.contains(" AS row_json FROM "), "{sql}");
            assert!(!sql.contains(';'), "{sql}");
            let upper = sql.to_ascii_uppercase();
            for word in upper.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                assert!(
                    !WRITE_WORDS.contains(&word),
                    "write keyword {word} in {sql}"
                );
            }
        }
    }
}
