use super::*;

#[test]
fn history_backup_round_trips_without_accounts_or_credentials() {
    let (_, fake, root) = setup();
    let plugin = MyWonderland::new(root.join("series-v2"), fake).unwrap();
    fs::create_dir_all(root.join("account/credentials")).unwrap();
    fs::write(
        root.join("account/credentials/token.bin"),
        b"synthetic-private-cookie",
    )
    .unwrap();
    let mut series = Series::empty(scope());
    series.fetch_count = 5;
    plugin.save(&series).unwrap();
    let backup = plugin.export_history().unwrap();
    assert!(!String::from_utf8_lossy(&backup).contains("synthetic-private-cookie"));
    let (target, _, target_root) = setup();
    assert_eq!(target.import_history(&backup).unwrap().created, 1);
    assert_eq!(target.series(&scope()).unwrap().unwrap().fetch_count, 5);
    assert_eq!(target.import_history(&backup).unwrap().unchanged, 1);
    assert!(!target_root.join("account").exists());
    assert_eq!(
        fs::read(root.join("account/credentials/token.bin")).unwrap(),
        b"synthetic-private-cookie"
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(target_root).unwrap();
}

#[test]
fn importing_an_old_series_preserves_newer_values_and_unions_history() {
    let (plugin, _, root) = setup();
    let mut current = Series::empty(scope());
    current.updated_at = 20;
    current.fetch_count = 4;
    current.overview = overview_of(vec![trend_sample("hot", &[("2026-10-02", "20")])]);
    plugin.save(&current).unwrap();
    let mut incoming = Series::empty(scope());
    incoming.updated_at = 10;
    incoming.fetch_count = 3;
    incoming.overview = overview_of(vec![trend_sample(
        "hot",
        &[("2026-10-01", "10"), ("2026-10-02", "0")],
    )]);
    let report = plugin
        .import_history(&serde_json::to_vec(&incoming).unwrap())
        .unwrap();
    assert_eq!(report.merged, 1);
    let combined = plugin.series(&scope()).unwrap().unwrap();
    assert_eq!(combined.updated_at, 20);
    assert_eq!(combined.fetch_count, 4);
    let points = &combined.overview.trend_data[0].daily_stats;
    assert_eq!(points.len(), 2);
    assert_eq!(points[1].cur, "20");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_batch_or_duplicate_scopes_cannot_partially_change_history() {
    let (plugin, _, root) = setup();
    let valid = Series::empty(scope());
    let mut invalid = valid.clone();
    invalid.scope.account_key = "../escape".into();
    for records in [
        vec![valid.clone(), invalid],
        vec![valid.clone(), valid.clone()],
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({"format":"wonderland-my-wonderland-history","schema_version":1,"series":records})).unwrap();
        assert!(plugin.import_history(&bytes).is_err());
        assert!(plugin.series(&scope()).unwrap().is_none());
    }
    assert_eq!(
        plugin
            .import_history(&serde_json::to_vec(&valid).unwrap())
            .unwrap()
            .created,
        1
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn importing_or_exporting_while_collecting_reports_busy() {
    let (plugin, _, root) = setup();
    let guard = plugin.collecting.try_lock().unwrap();
    assert!(matches!(plugin.export_history(), Err(PluginFailure::Busy)));
    assert!(matches!(
        plugin.import_history(b"{}"),
        Err(PluginFailure::Busy)
    ));
    drop(guard);
    fs::remove_dir_all(root).unwrap();
}
