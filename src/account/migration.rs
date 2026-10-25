//! One-time import of the former Core store. Original encrypted bytes are retained.
use super::{Account, AccountError};
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::path::PathBuf;
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Deserialize)]
struct Metadata {
    accounts: Vec<Account>,
    #[serde(default)]
    current_account: Option<String>,
}
fn failure(error: impl std::fmt::Display) -> AccountError {
    AccountError::CredentialStore(error.to_string())
}
pub fn validate_metadata(root: &Path) -> Result<(), AccountError> {
    match fs::read(root.join("account/accounts.json")) {
        Ok(bytes) => {
            let metadata: Metadata = serde_json::from_slice(&bytes).map_err(failure)?;
            let _ = (metadata.accounts, metadata.current_account);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure(error)),
    }
}
pub fn import_legacy(data: &Path, legacy_root: Option<&Path>) -> Result<bool, AccountError> {
    let Some(legacy) = legacy_root else {
        return Ok(false);
    };
    let source = legacy.join("account");
    let destination = data.join("account");
    let marker = source.join("my-wonderland-imported.json");
    if destination
        .join("accounts.json")
        .try_exists()
        .map_err(failure)?
    {
        validate_metadata(data)?;
        if source.join("accounts.json").try_exists().map_err(failure)?
            && !marker.try_exists().map_err(failure)?
        {
            mark_imported(&marker)?;
        }
        return Ok(false);
    }
    // Explicit plugin-data deletion must not resurrect credentials from the backup.
    if marker.try_exists().map_err(failure)? {
        return Ok(false);
    }
    if !source.join("accounts.json").try_exists().map_err(failure)? {
        return Ok(false);
    }
    validate_metadata(legacy)?;
    // A partial pre-existing destination needs recovery, never an automatic overwrite.
    if destination.try_exists().map_err(failure)? {
        return Err(failure(
            "账号目标目录已有未完成的数据，请先恢复；原账号目录保持不变",
        ));
    }
    fs::create_dir_all(data).map_err(failure)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = data.join(format!(".account-import-{}-{nonce}", std::process::id()));
    fs::create_dir(&staging).map_err(failure)?;
    let result = (|| {
        let mut digest = Sha256::new();
        copy_file(
            &source.join("accounts.json"),
            &staging.join("accounts.json"),
            &mut digest,
        )?;
        if source.join("current").try_exists().map_err(failure)? {
            copy_file(
                &source.join("current"),
                &staging.join("current"),
                &mut digest,
            )?;
        }
        let credential_dir = source.join("credentials");
        if credential_dir.try_exists().map_err(failure)? {
            fs::create_dir(staging.join("credentials")).map_err(failure)?;
            let mut entries = fs::read_dir(credential_dir)
                .map_err(failure)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(failure)?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                if !entry.file_type().map_err(failure)?.is_file() {
                    return Err(failure("旧凭据目录含非普通文件，已保留原文件"));
                }
                copy_file(
                    &entry.path(),
                    &staging.join("credentials").join(entry.file_name()),
                    &mut digest,
                )?;
            }
        }
        fs::write(staging.join("import-receipt.json"),serde_json::to_vec_pretty(&serde_json::json!({"format":1,"sourceSha256":format!("{:x}",digest.finalize()),"originalRetained":true})).map_err(failure)?).map_err(failure)?;
        fs::rename(&staging, &destination).map_err(failure)?;
        mark_imported(&marker)?;
        Ok(true)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}
fn mark_imported(path: &Path) -> Result<(), AccountError> {
    fs::write(
        path,
        b"{\"format\":1,\"owner\":\"wla_my_wonderland\",\"automaticImportCompleted\":true}\n",
    )
    .map_err(failure)
}
fn copy_file(source: &Path, destination: &Path, digest: &mut Sha256) -> Result<(), AccountError> {
    if !fs::symlink_metadata(source).map_err(failure)?.is_file() {
        return Err(failure("账号导入只接受普通文件"));
    }
    let bytes = fs::read(source).map_err(failure)?;
    fs::write(destination, &bytes).map_err(failure)?;
    if fs::read(destination).map_err(failure)? != bytes {
        return Err(failure("账号导入回读核验失败"));
    }
    digest.update(source.file_name().unwrap_or_default().as_encoded_bytes());
    digest.update(&bytes);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn preserves_current_account_and_large_encrypted_credentials_without_reimport() {
        use super::super::{
            AccountService, FsAccountService, StoredCookie, store::CredentialStore,
        };
        let (base, old, data) = roots();
        let a: Account = serde_json::from_value(
            serde_json::json!({"account_key":"a","game_roles":[],"updated_at":1}),
        )
        .unwrap();
        let b: Account = serde_json::from_value(
            serde_json::json!({"account_key":"b","game_roles":[],"updated_at":2}),
        )
        .unwrap();
        let store = CredentialStore::new(&old);
        let cookies = vec![StoredCookie {
            name: "ltoken".into(),
            value: "synthetic-token".repeat(2000),
            domain: ".mihoyo.com".into(),
            path: "/".into(),
        }];
        store.save_credentials("b", &cookies).unwrap();
        store.save_account_state(&[a, b], Some("b")).unwrap();
        let original = fs::read(old.join("account/credentials/b.bin")).unwrap();
        assert!(import_legacy(&data, Some(&old)).unwrap());
        let service = FsAccountService::load(data.clone()).unwrap();
        assert_eq!(service.snapshot().current_account_key.as_deref(), Some("b"));
        assert_eq!(
            CredentialStore::new(&data).load_credentials("b").unwrap(),
            cookies
        );
        assert_eq!(
            fs::read(data.join("account/credentials/b.bin")).unwrap(),
            original
        );
        service.remove_account("b").unwrap();
        assert!(!import_legacy(&data, Some(&old)).unwrap());
        assert_eq!(
            FsAccountService::load(data.clone())
                .unwrap()
                .snapshot()
                .accounts
                .len(),
            1
        );
        assert_eq!(store.load_accounts().len(), 2);
        assert_eq!(
            fs::read(old.join("account/credentials/b.bin")).unwrap(),
            original
        );
        fs::remove_dir_all(&data).unwrap();
        assert!(!import_legacy(&data, Some(&old)).unwrap());
        assert!(!data.join("account/accounts.json").exists());
        fs::remove_dir_all(base).unwrap();
    }
    fn roots() -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "wla-account-import-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let legacy = base.join("old");
        let data = base.join("plugin");
        fs::create_dir_all(legacy.join("account/credentials")).unwrap();
        (base, legacy, data)
    }
    #[test]
    fn import_is_byte_preserving_and_never_replays_deleted_accounts() {
        let (base, old, data) = roots();
        let bytes = b"{\"accounts\":[],\"current_account\":\"\"}";
        fs::write(old.join("account/accounts.json"), bytes).unwrap();
        fs::write(
            old.join("account/credentials/saved.bin.removing"),
            b"sealed-original",
        )
        .unwrap();
        assert!(import_legacy(&data, Some(&old)).unwrap());
        assert_eq!(fs::read(data.join("account/accounts.json")).unwrap(), bytes);
        assert_eq!(
            fs::read(data.join("account/credentials/saved.bin.removing")).unwrap(),
            b"sealed-original"
        );
        fs::write(data.join("account/accounts.json"), b"{\"accounts\":[]}").unwrap();
        assert!(!import_legacy(&data, Some(&old)).unwrap());
        assert_eq!(fs::read(old.join("account/accounts.json")).unwrap(), bytes);
        fs::remove_dir_all(base).unwrap()
    }
    #[test]
    fn malformed_source_does_not_create_empty_target() {
        let (base, old, data) = roots();
        fs::write(old.join("account/accounts.json"), b"{broken").unwrap();
        assert!(import_legacy(&data, Some(&old)).is_err());
        assert!(!data.join("account").exists());
        assert_eq!(
            fs::read(old.join("account/accounts.json")).unwrap(),
            b"{broken"
        );
        fs::remove_dir_all(base).unwrap()
    }
}
