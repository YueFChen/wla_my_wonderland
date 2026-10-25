use super::*;
use crate::Platform;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Data(PathBuf);
impl Data {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
                "wla-provider-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            )))
    }
}
impl Drop for Data {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cookies(platform: Platform, account_id: &str) -> Vec<StoredCookie> {
    let domain = match platform {
        Platform::Cn => ".mihoyo.com",
        Platform::Global => ".hoyoverse.com",
    };
    [
        ("ltuid_v2", account_id),
        ("ltoken_v2", "synthetic-test-token"),
    ]
    .into_iter()
    .map(|(name, value)| StoredCookie {
        name: name.into(),
        value: value.into(),
        domain: domain.into(),
        path: "/".into(),
    })
    .collect()
}

#[test]
fn same_official_id_has_independent_credentials_and_survives_restart_and_removal() {
    let data = Data::new();
    let account = FsAccountService::load(data.0.clone()).unwrap();
    account.begin_login(Platform::Cn).unwrap();
    account.submit_login(cookies(Platform::Cn, "123")).unwrap();
    account.begin_login(Platform::Global).unwrap();
    account
        .submit_login(cookies(Platform::Global, "123"))
        .unwrap();
    let store = super::store::CredentialStore::new(&data.0);
    assert_eq!(
        store.load_credentials("123").unwrap(),
        cookies(Platform::Cn, "123")
    );
    assert_eq!(
        store.load_credentials("global-123").unwrap(),
        cookies(Platform::Global, "123")
    );
    assert!(
        !fs::read(data.0.join("account/credentials/global-123.bin"))
            .unwrap()
            .windows(20)
            .any(|v| v == b"synthetic-test-token")
    );
    let restored = FsAccountService::load(data.0.clone()).unwrap();
    let snapshot = restored.snapshot();
    assert_eq!(snapshot.accounts.len(), 2);
    assert_eq!(snapshot.current_account_key.as_deref(), Some("global-123"));
    assert_eq!(snapshot.accounts[0].platform, Platform::Cn);
    assert_eq!(snapshot.accounts[1].platform, Platform::Global);
    assert_eq!(snapshot.accounts[1].account_id.as_deref(), Some("123"));
    restored.remove_account("global-123").unwrap();
    assert!(data.0.join("account/credentials/123.bin").exists());
    assert!(!data.0.join("account/credentials/global-123.bin").exists());
    assert_eq!(
        FsAccountService::load(data.0.clone())
            .unwrap()
            .snapshot()
            .accounts[0]
            .account_key,
        "123"
    );
}

#[test]
fn loading_old_metadata_preserves_exact_bytes_and_defaults_to_cn() {
    let data = Data::new();
    fs::create_dir_all(data.0.join("account")).unwrap();
    let bytes = br#"{"accounts":[{"account_key":"123","game_roles":[],"updated_at":1}],"current_account":"123"}"#;
    fs::write(data.0.join("account/accounts.json"), bytes).unwrap();
    let snapshot = FsAccountService::load(data.0.clone()).unwrap().snapshot();
    assert_eq!(snapshot.accounts[0].platform, Platform::Cn);
    assert_eq!(snapshot.current_account_key.as_deref(), Some("123"));
    assert_eq!(
        fs::read(data.0.join("account/accounts.json")).unwrap(),
        bytes
    );
}

#[test]
fn an_imported_key_collision_does_not_replace_the_original_credential() {
    let data = Data::new();
    let store = super::store::CredentialStore::new(&data.0);
    let old: Account = serde_json::from_value(
        serde_json::json!({"account_key":"global-123","game_roles":[],"updated_at":1}),
    )
    .unwrap();
    store
        .save_credentials("global-123", &cookies(Platform::Cn, "other"))
        .unwrap();
    store
        .save_account_state(&[old], Some("global-123"))
        .unwrap();
    let original = fs::read(data.0.join("account/credentials/global-123.bin")).unwrap();
    let account = FsAccountService::load(data.0.clone()).unwrap();
    account.begin_login(Platform::Global).unwrap();
    assert!(
        account
            .submit_login(cookies(Platform::Global, "123"))
            .is_err()
    );
    assert_eq!(
        fs::read(data.0.join("account/credentials/global-123.bin")).unwrap(),
        original
    );
}
