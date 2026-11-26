// Shared helpers for the SPEC-002a test binaries. Each file under tests/ is compiled as its
// own crate and uses a different subset of these helpers, so unused ones would warn per binary.
#![allow(dead_code)]

use backend_client::adapters::memory::{MasterDataFixture, MemoryClientStore};
use backend_client::master_data::{
    CardArtRow, CardRow, CategoryRow, DailyCardRow, DailyMetaRow, LoveStatusRow, PositionRow,
    SituationRow, SuggestionRow, TextMap,
};
use serde_json::Value;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub fn block_on<F: Future>(fut: F) -> F::Output {
    fn noop(_: *const ()) {}
    fn clone(ptr: *const ()) -> RawWaker {
        RawWaker::new(ptr, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut pinned = pin!(fut);
    loop {
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// Body limit 4 MiB: the largest master-data body is about 359 KB.
pub async fn body_text(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

pub async fn body_json(res: axum::response::Response) -> Value {
    serde_json::from_str(&body_text(res).await).unwrap()
}

fn s(text: &str) -> String {
    text.to_string()
}

fn texts(pairs: &[(&str, Option<&str>)]) -> TextMap {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
        .collect()
}

fn love_status(
    key: &str,
    daily_label: &str,
    reading_label: &str,
    daily: i64,
    reading: i64,
) -> LoveStatusRow {
    LoveStatusRow {
        key: s(key),
        daily_label: s(daily_label),
        reading_label: s(reading_label),
        daily_sort_order: daily,
        reading_sort_order: reading,
    }
}

fn daily_card(card_id: &str, name: &str, sort_order: i64, love_by_status: TextMap) -> DailyCardRow {
    DailyCardRow {
        card_id: s(card_id),
        name: s(name),
        arcana: s("major"),
        reading_mode: s("daily_single"),
        time_horizon: s("today"),
        orientation: s("upright_only"),
        content_status: s("assistant_draft_pending_review"),
        approved_for_web: false,
        theme: s("theme"),
        overview: s("overview"),
        work: s("work"),
        money: s("money"),
        rabbit_message: s("rabbit"),
        colors: s("สีเหลือง สีขาว"),
        affirmation: s("affirmation"),
        summary_theme: Some(s("summary")),
        overview_theme: None,
        sort_order,
        love_by_status,
    }
}

/// Small hand-built data set, rows stored in reverse order so the assembly's sorting is exercised.
/// Love statuses: single (daily 0, reading 0), ex_partner (daily 1, reading 2),
/// complicated (daily 2, reading 1). Cards: the-fool (art), six-of-wands (no art).
pub fn tiny_fixture() -> MasterDataFixture {
    MasterDataFixture {
        love_statuses: vec![
            love_status("complicated", "ความสัมพันธ์ซับซ้อน", "ซับซ้อน", 2, 1),
            love_status("ex_partner", "คนรักเก่า", "เลิกกัน", 1, 2),
            love_status("single", "คนโสด", "คนโสด", 0, 0),
        ],
        categories: vec![
            CategoryRow {
                key: s("work"),
                label: s("การงาน"),
                icon: s("☆"),
                hint: s("งานและอาชีพ"),
                sort_order: 1,
            },
            CategoryRow {
                key: s("love"),
                label: s("ความรัก"),
                icon: s("♡"),
                hint: s("ความรู้สึกและความสัมพันธ์"),
                sort_order: 0,
            },
        ],
        situations: vec![
            SituationRow {
                key: s("job_seeking"),
                category: Some(s("work")),
                label: s("มองหางานใหม่"),
                sort_order: 1,
            },
            SituationRow {
                key: s("current_job"),
                category: Some(s("work")),
                label: s("งานปัจจุบัน"),
                sort_order: 0,
            },
        ],
        suggestions: vec![
            SuggestionRow {
                id: s("work-1"),
                category: Some(s("work")),
                text: s("q-work"),
                sort_order: 0,
            },
            SuggestionRow {
                id: s("love-1"),
                category: Some(s("love")),
                text: s("q-love"),
                sort_order: 0,
            },
        ],
        positions: vec![
            PositionRow {
                key: s("future"),
                label: s("อนาคต"),
                sort_order: 2,
            },
            PositionRow {
                key: s("present"),
                label: s("ปัจจุบัน"),
                sort_order: 1,
            },
            PositionRow {
                key: s("past"),
                label: s("อดีต"),
                sort_order: 0,
            },
        ],
        cards: vec![
            CardRow {
                id: s("six-of-wands"),
                name: s("Six of Wands"),
                keyword: s("ชัยชนะ"),
                art: None,
                sort_order: 1,
                meanings: texts(&[("single", Some("m-single"))]),
                details: texts(&[("single", Some("d-single"))]),
            },
            CardRow {
                id: s("the-fool"),
                name: s("The Fool"),
                keyword: s("การเริ่มต้น"),
                art: Some(s("/cards/v3/the-fool-e11e1ee168a4.webp")),
                sort_order: 0,
                meanings: texts(&[
                    ("single", Some("m1")),
                    ("ex_partner", Some("m2")),
                    ("complicated", Some("m3")),
                ]),
                details: texts(&[
                    ("single", Some("d1")),
                    ("ex_partner", Some("d2")),
                    ("complicated", Some("d3")),
                ]),
            },
        ],
        card_art: vec![CardArtRow {
            id: s("the-fool"),
            art: s("/cards/v3/the-fool-e11e1ee168a4.webp"),
            width: 637,
            height: 1024,
            rendered_width: 600,
            rendered_height: 965,
            sha256: "a".repeat(64),
            bytes: 51_234,
            sort_order: 0,
        }],
        daily_meta: Some(DailyMetaRow {
            schema_version: 1,
            version: s("daily-v1-draft-2026-11-21"),
            updated: s("2026-11-21"),
            status: s("draft_pending_owner_review"),
            scope: s("daily"),
            auto_fallback_to_general_or_spread: true,
        }),
        daily_cards: vec![
            daily_card(
                "six-of-wands",
                "Six of Wands",
                1,
                texts(&[
                    ("single", Some("l1")),
                    ("ex_partner", Some("l2")),
                    ("complicated", Some("l3")),
                ]),
            ),
            daily_card(
                "the-fool",
                "The Fool",
                0,
                texts(&[
                    ("single", Some("l1")),
                    ("ex_partner", Some("l2")),
                    ("complicated", None),
                ]),
            ),
        ],
    }
}

pub fn tiny_store() -> Arc<MemoryClientStore> {
    let store = MemoryClientStore::new();
    *store.master_data.lock().unwrap() = tiny_fixture();
    store
}
