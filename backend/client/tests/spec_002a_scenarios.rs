mod support;

use axum::http::StatusCode;
use backend_client::adapters::memory::MemoryClientStore;
use backend_client::config::Environment;
use std::sync::atomic::Ordering;
use support::{block_on, tiny_store};

#[test]
fn memory_port_returns_fixture_rows_and_counts_reads() {
    block_on(async {
        let store = tiny_store();
        let ports = store.into_ports(Environment::Staging);
        assert_eq!(store.master_data_reads.load(Ordering::SeqCst), 0);

        let cards = ports.master_data.reading_cards().await.unwrap();
        assert_eq!(cards.cards.len(), 2);
        assert_eq!(cards.love_statuses.len(), 3);
        let daily = ports.master_data.daily_cards().await.unwrap();
        assert_eq!(daily.cards.len(), 2);
        assert!(daily.meta.auto_fallback_to_general_or_spread);
        assert_eq!(ports.master_data.card_art().await.unwrap().len(), 1);
        let config = ports.master_data.reading_config().await.unwrap();
        assert_eq!(config.categories.len(), 2);
        assert_eq!(config.positions.len(), 3);

        assert_eq!(store.master_data_reads.load(Ordering::SeqCst), 4);
    });
}

#[test]
fn memory_port_reports_d1_outage_as_503_on_every_method() {
    block_on(async {
        let store = tiny_store();
        store.d1_healthy.store(false, Ordering::SeqCst);
        let ports = store.into_ports(Environment::Staging);
        let statuses = [
            ports.master_data.reading_cards().await.unwrap_err().status,
            ports.master_data.daily_cards().await.unwrap_err().status,
            ports.master_data.card_art().await.unwrap_err().status,
            ports.master_data.reading_config().await.unwrap_err().status,
        ];
        assert_eq!(statuses, [StatusCode::SERVICE_UNAVAILABLE; 4]);
    });
}

#[test]
fn memory_port_without_daily_meta_is_503_and_empty_store_reads_empty_lists() {
    block_on(async {
        let store = MemoryClientStore::new();
        let ports = store.into_ports(Environment::Staging);
        let err = ports.master_data.daily_cards().await.unwrap_err();
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(ports
            .master_data
            .reading_cards()
            .await
            .unwrap()
            .cards
            .is_empty());
        assert!(ports.master_data.card_art().await.unwrap().is_empty());
    });
}
