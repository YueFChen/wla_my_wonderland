use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Mutex as StdMutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use wonderland_plugin_sdk::PluginFailure;
fn fixture(name: &str) -> serde_json::Value {
    let source = match name {
        "overview" => include_str!("../tests/fixtures/overview.json"),
        "page1" => include_str!("../tests/fixtures/page1.json"),
        "page2" => include_str!("../tests/fixtures/page2.json"),
        "detail" => include_str!("../tests/fixtures/detail.json"),
        "empty" => include_str!("../tests/fixtures/empty.json"),
        "expired" => include_str!("../tests/fixtures/expired.json"),
        _ => include_str!("../tests/fixtures/business.json"),
    };
    serde_json::from_str(source).unwrap()
}
fn scope() -> Scope {
    Scope {
        account_key: "demo".into(),
        uid: "100001".into(),
        region: "cn_gf01".into(),
    }
}
struct Fake {
    fail: AtomicBool,
    fail_detail: StdMutex<Option<String>>,
    calls: StdMutex<Vec<String>>,
}
impl AccountProvider for Fake {
    fn snapshot(&self) -> AccountSnapshot {
        AccountSnapshot {
            status: AccountStatus::LoggedIn,
            current_account_key: Some("demo".into()),
            accounts: vec![Account {
                mid: None,
                avatar_url: None,
                creator_level: None,
                creator_exp: None,
                creator_exp_total: None,
                updated_at: 0,
                account_key: "demo".into(),
                game_roles: vec![GameRole {
                    uid: "100001".into(),
                    region: "cn_gf01".into(),
                    region_name: "天空岛".into(),
                    nickname: "示例".into(),
                    level: 60,
                }],
            }],
            last_login_failure: None,
        }
    }
    fn authed_get<'a>(
        &'a self,
        _: &'a str,
        path: &'a str,
        query: &'a [(String, String)],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, PluginFailure>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(path.to_owned());
            let mut value = if path.ends_with("get_user_info") {
                fixture("overview")
            } else if path.ends_with("get_stage_list") {
                if query.iter().any(|(k, v)| k == "page_index" && v == "2") {
                    if self.fail.load(Ordering::SeqCst) {
                        return Err(PluginFailure::Timeout);
                    }
                    fixture("page2")
                } else {
                    fixture("page1")
                }
            } else {
                let stage_id = query
                    .iter()
                    .find(|(key, _)| key == "stage_id")
                    .map(|(_, value)| value.clone())
                    .unwrap();
                if self.fail_detail.lock().unwrap().as_ref() == Some(&stage_id) {
                    return Err(PluginFailure::Timeout);
                }
                fixture("detail")
            };
            if path.ends_with("get_stage_detail") {
                value["data"]["stage_info"]["stage_id"] = query
                    .iter()
                    .find(|(k, _)| k == "stage_id")
                    .unwrap()
                    .1
                    .clone()
                    .into()
            }
            Ok(serde_json::to_vec(&value).unwrap())
        })
    }
}
static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn setup() -> (MyWonderland, Arc<Fake>, PathBuf) {
    setup_at(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    )
}

fn setup_at(timestamp: u128) -> (MyWonderland, Arc<Fake>, PathBuf) {
    let temp = std::env::temp_dir().join(format!(
        "wonderland-test-{}-{}-{}",
        std::process::id(),
        timestamp,
        TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let fake = Arc::new(Fake {
        fail: AtomicBool::new(false),
        fail_detail: StdMutex::new(None),
        calls: StdMutex::new(vec![]),
    });
    let p = MyWonderland::new(temp.clone(), fake.clone()).unwrap();
    (p, fake, temp)
}
#[test]
fn fixtures_with_identical_clock_ticks_keep_data_and_cleanup_isolated() {
    let (first, _, first_path) = setup_at(123);
    let (second, _, second_path) = setup_at(123);
    assert_ne!(first_path, second_path);

    first.save(&Series::empty(scope())).unwrap();
    assert!(second.series(&scope()).unwrap().is_none());
    second.save(&Series::empty(scope())).unwrap();
    fs::remove_dir_all(first_path).unwrap();
    assert!(second.series(&scope()).unwrap().is_some());
    fs::remove_dir_all(second_path).unwrap();
}

fn trend_sample(metric: &str, points: &[(&str, &str)]) -> Trend {
    Trend {
        metric_type: metric.into(),
        delta_7_day: String::new(),
        delta_7_day_invalid: false,
        delta_30_day: String::new(),
        delta_30_day_invalid: false,
        daily_stats: points
            .iter()
            .map(|(date, cur)| DailyStat {
                date: (*date).into(),
                cur: (*cur).into(),
                delta: String::new(),
                value_type: "METRIC_VALUE_TYPE_NUMBER".into(),
                cur_invalid: false,
            })
            .collect(),
        value_type: "METRIC_VALUE_TYPE_NUMBER".into(),
        calculate_type: String::new(),
        normalized_stats_7d: Vec::new(),
        multi_group_dist: None,
    }
}
fn overview_of(trends: Vec<Trend>) -> Overview {
    Overview {
        today_str: "2026-01-05".into(),
        today_stats: Vec::new(),
        trend_data: trends,
    }
}
fn stage_info(id: &str, status: &str) -> StageInfo {
    StageInfo {
        uid: scope().uid,
        region: scope().region,
        stage_id: id.into(),
        stage_name: format!("作品 {id}"),
        cover_url: String::new(),
        latest_online_time: 0,
        game_list_status: status.into(),
    }
}
fn stage_of(id: &str, status: &str) -> Stage {
    Stage {
        base_info: stage_info(id, status),
        today_stats: Vec::new(),
    }
}
fn detail_of(id: &str, points: &[(&str, &str)]) -> Detail {
    Detail {
        today_str: "2026-01-05".into(),
        stage_info: stage_info(id, "GAME_LIST_STATUS_ONLINE"),
        today_stats: Vec::new(),
        trend_data: vec![trend_sample("METRIC_STAGE_TYPE_STAGE_HOT_SCORE", points)],
        metric_groups: Vec::new(),
        comment_module_info: Comments::default(),
    }
}
fn point_count(series: &Series) -> usize {
    series
        .overview
        .trend_data
        .first()
        .map(|t| t.daily_stats.len())
        .unwrap_or(0)
}
#[test]
fn protocol_and_empty_page() {
    assert!(matches!(
        decode::<Overview>(fixture("expired")),
        Err(PluginFailure::SessionExpired)
    ));
    assert!(matches!(
        decode::<Overview>(fixture("business")),
        Err(PluginFailure::Business { code: 12345, .. })
    ));
    assert!(matches!(
        decode::<Overview>(serde_json::json!({"retcode":0,"data":{}})),
        Err(PluginFailure::InvalidResponse)
    ));
    let empty = decode::<StagePage>(fixture("empty")).unwrap();
    assert!(
        append_page(
            &mut vec![],
            &mut BTreeSet::new(),
            &mut None,
            empty,
            &scope()
        )
        .unwrap()
    );
}
#[test]
fn paging_duplicate_and_total_changes_rejected() {
    let mut list = vec![];
    let mut seen = BTreeSet::new();
    let mut total = None;
    let p = decode::<StagePage>(fixture("page1")).unwrap();
    assert!(!append_page(&mut list, &mut seen, &mut total, p.clone(), &scope()).unwrap());
    assert!(append_page(&mut list, &mut seen, &mut total, p, &scope()).is_err());
    let mut changed = decode::<StagePage>(fixture("page2")).unwrap();
    changed.total = 10;
    assert!(append_page(&mut list, &mut seen, &mut total, changed, &scope()).is_err());
}
#[tokio::test]
async fn collect_merges_into_one_deduped_series() {
    let (p, _, temp) = setup();
    let first = p.collect(scope()).await.unwrap();
    assert_eq!(first.fetch_count, 1);
    assert_eq!(first.stages.len(), 9);
    assert_eq!(first.details.len(), 9);
    assert_eq!(
        first
            .stages
            .iter()
            .map(|stage| stage.base_info.stage_id.clone())
            .collect::<Vec<_>>(),
        (1000..1009).map(|id| id.to_string()).collect::<Vec<_>>()
    );
    assert!(point_count(&first) > 0);

    // 第二次采集拿到同一个滚动窗口：点位按日期去重，不产生重复。
    let second = p.collect(scope()).await.unwrap();
    assert_eq!(second.fetch_count, 2);
    assert_eq!(point_count(&second), point_count(&first));
    assert_eq!(second.stages.len(), 9);

    let stored = p.series(&scope()).unwrap().unwrap();
    assert_eq!(stored.fetch_count, 2);
    assert_eq!(stored.details.len(), 9);
    fs::remove_dir_all(temp).unwrap();
}
#[tokio::test]
async fn detail_failure_keeps_successful_collection_data() {
    let (plugin, fake, temp) = setup();
    *fake.fail_detail.lock().unwrap() = Some("1002".into());

    let error = plugin.collect(scope()).await.unwrap_err();
    assert!(matches!(error, PluginFailure::Other(_)));

    let stored = plugin.series(&scope()).unwrap().unwrap();
    assert_eq!(stored.stages.len(), 9);
    assert_eq!(stored.details.len(), 8);
    assert!(!stored.details.contains_key("1002"));
    assert!(stored.details.contains_key("1008"));
    assert!(point_count(&stored) > 0);
    fs::remove_dir_all(temp).unwrap();
}
#[tokio::test]
async fn failed_collect_keeps_previous_series() {
    let (p, fake, temp) = setup();
    let ok = p.collect(scope()).await.unwrap();
    fake.fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        p.collect(scope()).await,
        Err(PluginFailure::Timeout)
    ));
    // 半途失败不落盘：序列仍是上一次成功的结果。
    let stored = p.series(&scope()).unwrap().unwrap();
    assert_eq!(stored.fetch_count, ok.fetch_count);
    assert_eq!(stored.updated_at, ok.updated_at);

    let mut other = scope();
    other.account_key = "other".into();
    assert!(p.series(&other).unwrap().is_none());
    assert!(matches!(
        p.collect(other).await,
        Err(PluginFailure::AccountNotFound(_))
    ));
    fs::remove_dir_all(temp).unwrap();
}
#[test]
fn merge_dedups_points_and_keeps_works_and_metrics() {
    let mut series = Series::empty(scope());
    // 第一次：指标 M1/M2，作品 A（在线）+ B（离线）。
    series.merge(
        1_000,
        overview_of(vec![
            trend_sample("M1", &[("2026-01-01", "1"), ("2026-01-02", "2")]),
            trend_sample("M2", &[("2026-01-01", "5")]),
        ]),
        vec![
            stage_of("100001", "GAME_LIST_STATUS_ONLINE"),
            stage_of("100002", "GAME_LIST_STATUS_OFFLINE"),
        ],
        BTreeMap::from([
            (
                "100001".to_string(),
                detail_of("100001", &[("2026-01-01", "7")]),
            ),
            (
                "100002".to_string(),
                detail_of("100002", &[("2026-01-01", "9")]),
            ),
        ]),
    );
    // 第二次：官方窗口里只剩 M1 与作品 A，且 01-02 的值被修订。
    series.merge(
        2_000,
        overview_of(vec![trend_sample("M1", &[("2026-01-02", "3")])]),
        vec![stage_of("100001", "GAME_LIST_STATUS_ONLINE")],
        BTreeMap::from([(
            "100001".to_string(),
            detail_of("100001", &[("2026-01-02", "8")]),
        )]),
    );

    assert_eq!(series.fetch_count, 2);
    assert_eq!(series.updated_at, 2_000);
    // 指标并集：只在旧窗口出现过的 M2 仍保留。
    assert_eq!(series.overview.trend_data.len(), 2);
    let m1 = series
        .overview
        .trend_data
        .iter()
        .find(|t| t.metric_type == "M1")
        .unwrap();
    assert_eq!(m1.daily_stats.len(), 2, "同一天不会重复存储");
    assert_eq!(m1.daily_stats[1].cur, "3", "同一天以较新的采集为准");
    // 作品并集：离开窗口的 B 仍保留，且它的历史点位在。
    assert_eq!(series.stages.len(), 2);
    assert_eq!(series.details.len(), 2);
    assert_eq!(series.details["100002"].trend_data[0].daily_stats.len(), 1);
    assert_eq!(series.details["100001"].trend_data[0].daily_stats.len(), 2);
}
#[test]
fn merge_keeps_api_order_and_appends_archived_works() {
    let mut series = Series::empty(scope());
    series.merge(
        1_000,
        overview_of(vec![]),
        vec![
            stage_of("old-a", "GAME_LIST_STATUS_OFFLINE"),
            stage_of("old-b", "GAME_LIST_STATUS_OFFLINE"),
            stage_of("archived", "GAME_LIST_STATUS_DELETED"),
        ],
        BTreeMap::new(),
    );

    series.merge(
        2_000,
        overview_of(vec![]),
        vec![
            stage_of("new", "GAME_LIST_STATUS_ONLINE"),
            stage_of("old-b", "GAME_LIST_STATUS_ONLINE"),
            stage_of("old-a", "GAME_LIST_STATUS_ONLINE"),
        ],
        BTreeMap::new(),
    );

    assert_eq!(
        series
            .stages
            .iter()
            .map(|stage| stage.base_info.stage_id.as_str())
            .collect::<Vec<_>>(),
        ["new", "old-b", "old-a", "archived"]
    );
}
#[test]
fn merge_preserves_invalid_points_and_latest_corrections() {
    let mut series = Series::empty(scope());
    let mut first = trend_sample("M1", &[("2026-01-01", "10"), ("2026-01-02", "")]);
    first.daily_stats[1].cur_invalid = true;
    series.merge(1_000, overview_of(vec![first]), vec![], BTreeMap::new());
    // 窗口滑动后 01-02 不再返回：无效点与标记都不应被抹掉。
    series.merge(
        2_000,
        overview_of(vec![trend_sample("M1", &[("2026-01-03", "12")])]),
        vec![],
        BTreeMap::new(),
    );
    let m1 = &series.overview.trend_data[0];
    assert_eq!(m1.daily_stats.len(), 3);
    assert!(m1.daily_stats[1].cur_invalid);
    assert_eq!(m1.daily_stats[1].cur, "");
    // 官方修订同一天的值时，以较新者为准。
    series.merge(
        3_000,
        overview_of(vec![trend_sample("M1", &[("2026-01-03", "99")])]),
        vec![],
        BTreeMap::new(),
    );
    assert_eq!(
        series.overview.trend_data[0]
            .daily_stats
            .last()
            .unwrap()
            .cur,
        "99"
    );
}
#[test]
fn scope_paths_and_busy() {
    let (p, _, temp) = setup();
    let mut bad = scope();
    bad.uid = "../other".into();
    assert!(p.series_path(&bad).is_err());
    assert!(safe("cn_gf01"));
    assert!(!safe(""));
    assert!(!safe("a/b"));
    let _lock = p.collecting.try_lock().unwrap();
    assert!(p.collecting.try_lock().is_err());
    fs::remove_dir_all(temp).unwrap();
}
