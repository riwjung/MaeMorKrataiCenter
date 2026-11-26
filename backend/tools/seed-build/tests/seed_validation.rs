//! Each check of the seed build, triggered by a one-field edit of the committed seed.

use seed_build::{build_sql, SeedSources};
use serde_json::{json, Value};
use std::path::Path;

fn committed() -> SeedSources {
    SeedSources::read_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../seed"))
        .expect("seed files are readable")
}

fn edit(text: &str, change: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(text).expect("seed file is JSON");
    change(&mut value);
    serde_json::to_string_pretty(&value).expect("serialize")
}

fn assert_fails(sources: &SeedSources, needle: &str) {
    let err = build_sql(sources).expect_err("the build must fail");
    assert!(err.contains(needle), "expected `{needle}` in: {err}");
}

#[test]
fn the_committed_seed_builds() {
    assert!(build_sql(&committed()).is_ok());
}

#[test]
fn a_dropped_reading_card_field_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| {
        v[0]["source_url"] = json!("https://example.invalid/x")
    });
    assert_fails(&s, "source_url");
}

#[test]
fn a_dropped_daily_source_object_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| {
        v["cards"][0]["source"] = json!({"raw_file": "raw/x.json"})
    });
    assert_fails(&s, "source");
}

#[test]
fn a_dropped_manifest_field_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| {
        v["cards"][0]["sourceFile"] = json!("the-fool.jpg")
    });
    assert_fails(&s, "sourceFile");
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| {
        v["capturedAt"] = json!("2026-10-30T04:43:42.890Z")
    });
    assert_fails(&s, "capturedAt");
}

#[test]
fn a_chatgpt_link_inside_text_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| {
        v[0]["meanings"]["single"] = json!("see https://chatgpt.com/share/x")
    });
    assert_fails(&s, "chatgpt.com");
}

#[test]
fn an_unknown_field_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[0]["notes"] = json!("x"));
    assert_fails(&s, "unknown field `notes`");
}

#[test]
fn a_control_character_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[0]["keyword"] = json!("a\u{0007}b"));
    assert_fails(&s, "control character U+0007");
}

#[test]
fn a_missing_love_status_key_in_meanings_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| {
        v[0]["meanings"]
            .as_object_mut()
            .expect("object")
            .remove("single");
    });
    assert_fails(&s, "meanings keys");
}

#[test]
fn an_unknown_love_status_in_daily_texts_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| {
        v["cards"][0]["content"]["love_by_status"]["widowed"] = json!("x")
    });
    assert_fails(&s, "content.love_by_status keys");
}

#[test]
fn missing_fields_that_disagree_with_null_texts_fail() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| v["cards"][0]["missing_fields"] = json!([]));
    assert_fails(&s, "missing_fields");
}

#[test]
fn an_art_entry_that_differs_from_its_card_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| {
        v["cards"][0]["art"] = json!("/cards/v3/other.webp")
    });
    assert_fails(&s, "art differs");
}

#[test]
fn an_art_entry_for_a_card_without_art_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| {
        let mut entry = v["cards"][0].clone();
        entry["id"] = json!("six-of-wands");
        v["cards"].as_array_mut().expect("array").push(entry);
    });
    assert_fails(&s, "art differs");
}

#[test]
fn an_art_entry_without_a_card_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| v["cards"][0]["id"] = json!("x-card"));
    assert_fails(&s, "has no card");
}

#[test]
fn a_daily_entry_without_a_card_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| v["cards"][0]["card_id"] = json!("x-card"));
    assert_fails(&s, "has no card");
}

#[test]
fn a_duplicate_card_id_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[1]["id"] = v[0]["id"].clone());
    assert_fails(&s, "duplicate card id");
}

#[test]
fn love_situations_that_differ_from_love_statuses_fail() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["situations"]["love"]
            .as_array_mut()
            .expect("array")
            .swap(0, 1);
    });
    assert_fails(&s, "situations.love");
}

#[test]
fn uncategorized_items_in_the_seed_fail() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["uncategorized_suggestions"] = json!([{"id": "x-1", "text": "x"}]);
    });
    assert_fails(&s, "must be empty");
}

#[test]
fn a_daily_label_that_differs_between_files_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| {
        v["love_statuses"][0]["source_label"] = json!("x")
    });
    assert_fails(&s, "source_label differs");
}

#[test]
fn a_situation_under_an_unknown_category_fails() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["situations"]["nope"] = json!([{"key": "k", "label": "l"}]);
    });
    assert_fails(&s, "unknown category `nope`");
}

#[test]
fn a_duplicate_suggestion_id_fails() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["suggestions"]["work"][0]["id"] = json!("love-1");
    });
    assert_fails(&s, "duplicate suggestion id `love-1`");
}
