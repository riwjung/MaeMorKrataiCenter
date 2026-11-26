//! Runs the client's master-data SQL on a real local D1 (SQLite) with the real migrations and
//! seed, and feeds the rows through the same parse and assembly code the Worker uses. Needs
//! Node.js (`npx wrangler`) and `make seed-build`; run it with `make test-d1-read`.

mod seed_support;

use backend_client::adapters::d1_master_data::{
    card_art_from_sets, daily_from_sets, reading_cards_from_sets, reading_config_from_sets,
    CARD_ART_BATCH, DAILY_CARDS_BATCH, READING_CARDS_BATCH, READING_CONFIG_BATCH,
};
use backend_client::master_data::{
    assemble_card_art, assemble_daily_cards, assemble_reading_cards, assemble_reading_config,
};
use d1_local::{backend_dir, D1Local};
use seed_support::{
    assert_a1_reading_config, assert_art_consistent, assert_card_art_matches_seed,
    assert_cards_match_seed, assert_clean, assert_daily_matches_seed,
    assert_reading_config_matches_seed, A1_SQL,
};

/// The `row_json` strings of each statement, as `D1Database::batch` would return them.
fn row_sets(db: &D1Local, statements: &[&str]) -> Vec<Vec<String>> {
    statements
        .iter()
        .map(|&sql| {
            db.query(sql)
                .iter()
                .map(|row| {
                    row["row_json"]
                        .as_str()
                        .unwrap_or_else(|| panic!("row without TEXT row_json: {row:?}"))
                        .to_string()
                })
                .collect()
        })
        .collect()
}

fn seeded_database() -> D1Local {
    let seed_sql = backend_dir().join("seed").join("build").join("seed.sql");
    assert!(
        seed_sql.exists(),
        "{} is missing; run `make seed-build` (make test-d1-read does)",
        seed_sql.display()
    );
    let db = D1Local::migrated("client_master_data_sql");
    db.exec_file(&seed_sql)
        .unwrap_or_else(|e| panic!("seed load failed: {e}"));
    db
}

#[test]
#[ignore = "needs Node.js and a seed build; run with `make test-d1-read`"]
fn d1_master_data_sql_on_real_sqlite_matches_the_seed() {
    let db = seeded_database();

    let cards = assemble_reading_cards(
        reading_cards_from_sets(row_sets(&db, READING_CARDS_BATCH)).expect("cards rows parse"),
    );
    let cards = serde_json::to_value(&cards).unwrap();
    assert_cards_match_seed(&cards);

    let daily = assemble_daily_cards(
        daily_from_sets(row_sets(&db, DAILY_CARDS_BATCH)).expect("daily rows parse"),
    );
    let daily_text = serde_json::to_string(&daily).unwrap();
    assert_daily_matches_seed(&serde_json::from_str(&daily_text).unwrap());
    assert_clean("daily (D1)", &daily_text);

    let art = assemble_card_art(
        card_art_from_sets(row_sets(&db, CARD_ART_BATCH)).expect("art rows parse"),
    );
    let art = serde_json::to_value(&art).unwrap();
    assert_card_art_matches_seed(&art);
    assert_art_consistent(&cards, &art);

    let config = assemble_reading_config(
        reading_config_from_sets(row_sets(&db, READING_CONFIG_BATCH)).expect("config rows parse"),
    );
    assert_reading_config_matches_seed(&serde_json::to_value(&config).unwrap());

    // Scenario A1 as direct SQL on the same database (FKs are enforced by D1).
    db.exec(A1_SQL).expect("the A1 fixture applies");
    let config = assemble_reading_config(
        reading_config_from_sets(row_sets(&db, READING_CONFIG_BATCH)).expect("config rows parse"),
    );
    assert_a1_reading_config(&serde_json::to_value(&config).unwrap());
}
