//! Portable series-v2 history. Accounts, cookies and browser profiles never enter a backup.
use crate::{MyWonderland, SERIES_VERSION, Series, scope_valid, storage};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};
use wonderland_plugin_sdk::PluginFailure;

const FORMAT: &str = "wonderland-my-wonderland-history";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    format: String,
    schema_version: u32,
    series: Vec<Series>,
}

#[derive(Debug, Default, Serialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct HistoryImportReport {
    pub created: u32,
    pub merged: u32,
    pub unchanged: u32,
}

fn invalid() -> PluginFailure {
    PluginFailure::LocalData("历史文件格式、角色范围或目录结构无效；原记录保持不变".into())
}

fn validate(series: &Series) -> Result<(), PluginFailure> {
    scope_valid(&series.scope)?;
    if series.schema_version != SERIES_VERSION
        || series.stages.iter().any(|stage| {
            stage.base_info.uid != series.scope.uid || stage.base_info.region != series.scope.region
        })
        || series.details.iter().any(|(id, detail)| {
            id != &detail.stage_info.stage_id
                || detail.stage_info.uid != series.scope.uid
                || detail.stage_info.region != series.scope.region
        })
    {
        return Err(invalid());
    }
    Ok(())
}

fn regular(path: &Path, directory: bool) -> Result<(), PluginFailure> {
    let metadata = fs::symlink_metadata(path).map_err(storage)?;
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(invalid());
    }
    Ok(())
}

fn history_files(root: &Path) -> Result<Vec<std::path::PathBuf>, PluginFailure> {
    let mut result = Vec::new();
    if !root.try_exists().map_err(storage)? {
        return Ok(result);
    }
    regular(root, true)?;
    for account in fs::read_dir(root).map_err(storage)? {
        let account = account.map_err(storage)?.path();
        regular(&account, true)?;
        for region in fs::read_dir(&account).map_err(storage)? {
            let region = region.map_err(storage)?.path();
            regular(&region, true)?;
            for file in fs::read_dir(&region).map_err(storage)? {
                let file = file.map_err(storage)?.path();
                if file
                    .extension()
                    .is_some_and(|extension| extension == "json")
                {
                    regular(&file, false)?;
                    result.push(file);
                }
            }
        }
    }
    result.sort();
    Ok(result)
}

impl MyWonderland {
    pub fn export_history(&self) -> Result<Vec<u8>, PluginFailure> {
        let _guard = self
            .collecting
            .try_lock()
            .map_err(|_| PluginFailure::Busy)?;
        let mut series = Vec::new();
        for file in history_files(&self.root)? {
            let record: Series = serde_json::from_slice(&fs::read(&file).map_err(storage)?)
                .map_err(|_| invalid())?;
            validate(&record)?;
            if self.series_path(&record.scope)? != file {
                return Err(invalid());
            }
            series.push(record);
        }
        serde_json::to_vec(&Backup {
            format: FORMAT.into(),
            schema_version: 1,
            series,
        })
        .map_err(storage)
    }

    pub fn import_history(&self, bytes: &[u8]) -> Result<HistoryImportReport, PluginFailure> {
        let _guard = self
            .collecting
            .try_lock()
            .map_err(|_| PluginFailure::Busy)?;
        regular(&self.root, true)?;
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        let records = if value.get("format").is_some() {
            let backup: Backup = serde_json::from_value(value).map_err(|_| invalid())?;
            if backup.format != FORMAT || backup.schema_version != 1 {
                return Err(invalid());
            }
            backup.series
        } else {
            vec![serde_json::from_value::<Series>(value).map_err(|_| invalid())?]
        };
        let mut seen = BTreeSet::new();
        let mut updates = Vec::new();
        let mut report = HistoryImportReport::default();
        // Validate the entire input and every existing destination before changing any file.
        for incoming in records {
            validate(&incoming)?;
            let path = self.series_path(&incoming.scope)?;
            if !seen.insert(path.clone()) {
                return Err(invalid());
            }
            for parent in [
                path.parent().unwrap().parent().unwrap(),
                path.parent().unwrap(),
            ] {
                if parent.try_exists().map_err(storage)? {
                    regular(parent, true)?;
                }
            }
            let previous = if path.try_exists().map_err(storage)? {
                regular(&path, false)?;
                Some(fs::read(&path).map_err(storage)?)
            } else {
                None
            };
            let combined = if let Some(bytes) = &previous {
                let current: Series = serde_json::from_slice(bytes).map_err(|_| invalid())?;
                validate(&current)?;
                if current.scope != incoming.scope {
                    return Err(invalid());
                }
                // An identical backup must retain upstream summaries as well as stored points.
                // Re-running merge would normalize summaries on untouched first-fetch details.
                if serde_json::to_value(&current).map_err(storage)?
                    == serde_json::to_value(&incoming).map_err(storage)?
                {
                    report.unchanged += 1;
                    continue;
                }
                let count = current.fetch_count.max(incoming.fetch_count);
                // Equal timestamps retain the existing record's authoritative values.
                let (mut older, newer) = if current.updated_at < incoming.updated_at {
                    (current, incoming)
                } else {
                    (incoming, current)
                };
                older.fetch_count = 0;
                older.merge(
                    newer.updated_at,
                    newer.overview,
                    newer.stages,
                    newer.details,
                );
                older.fetch_count = count;
                older
            } else {
                incoming
            };
            if previous.as_ref().is_some_and(|bytes| {
                serde_json::from_slice::<serde_json::Value>(bytes).ok()
                    == serde_json::to_value(&combined).ok()
            }) {
                report.unchanged += 1;
            } else {
                if previous.is_some() {
                    report.merged += 1;
                } else {
                    report.created += 1;
                }
                updates.push((path, combined, previous));
            }
        }
        for (index, (_, series, _)) in updates.iter().enumerate() {
            if let Err(error) = self.save(series) {
                for (path, _, previous) in updates[..index].iter().rev() {
                    if let Some(bytes) = previous {
                        let temporary = path.with_extension("import-rollback");
                        fs::write(&temporary, bytes)
                            .and_then(|()| fs::rename(temporary, path))
                            .map_err(storage)?;
                    } else {
                        fs::remove_file(path).map_err(storage)?;
                    }
                }
                return Err(error);
            }
        }
        Ok(report)
    }
}
