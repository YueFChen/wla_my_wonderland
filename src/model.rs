use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 本地序列的 schema 版本。旧版逐次快照是 v1，两者目录并存但互不读取。
pub const SERIES_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Scope {
    pub account_key: String,
    pub uid: String,
    pub region: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Stat {
    pub metric_type: String,
    pub cur: String,
    #[serde(default)]
    pub cur_invalid: bool,
    #[serde(default)]
    pub delta: String,
    #[serde(default)]
    pub delta_invalid: bool,
    pub value_type: String,
    #[serde(default)]
    pub calculate_type: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct DailyStat {
    pub date: String,
    pub cur: String,
    #[serde(default)]
    pub delta: String,
    pub value_type: String,
    #[serde(default)]
    pub cur_invalid: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Trend {
    pub metric_type: String,
    #[serde(default)]
    pub delta_7_day: String,
    #[serde(default)]
    pub delta_7_day_invalid: bool,
    #[serde(default)]
    pub delta_30_day: String,
    #[serde(default)]
    pub delta_30_day_invalid: bool,
    pub daily_stats: Vec<DailyStat>,
    pub value_type: String,
    #[serde(default)]
    pub calculate_type: String,
    #[serde(default)]
    pub normalized_stats_7d: Vec<DailyStat>,
    #[serde(default)]
    pub multi_group_dist: Option<Distribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct StageInfo {
    pub uid: String,
    pub region: String,
    pub stage_id: String,
    pub stage_name: String,
    pub cover_url: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub latest_online_time: i64,
    pub game_list_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Stage {
    pub base_info: StageInfo,
    pub today_stats: Vec<Stat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Overview {
    pub today_str: String,
    pub today_stats: Vec<Stat>,
    pub trend_data: Vec<Trend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct MetricGroup {
    pub group_type: String,
    pub metric_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Comment {
    pub comment_id: String,
    pub nickname: String,
    pub content: String,
    pub is_recommend: bool,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub comment_unix: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Comments {
    #[serde(default)]
    pub total_comment_num: String,
    #[serde(default)]
    pub comment_list: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Detail {
    pub today_str: String,
    pub stage_info: StageInfo,
    pub today_stats: Vec<Stat>,
    pub trend_data: Vec<Trend>,
    #[serde(default)]
    pub metric_groups: Vec<MetricGroup>,
    #[serde(default)]
    pub comment_module_info: Comments,
}

/// 本地长期序列：每个角色一份，点位按日期去重。
///
/// 官方只给滚动窗口，相邻两次采集会大量重叠，所以不保存逐次快照与原始响应，
/// 只保留去重后的点位 + 最新元数据（作品取并集，离开窗口的也留着）。
/// 旧的逐次快照（v1 目录）与之并存但不再读取。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Series {
    pub schema_version: u32,
    pub scope: Scope,
    /// 最近一次采集时间（Unix 秒）。
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub updated_at: u64,
    /// 累计采集次数。
    pub fetch_count: u32,
    pub overview: Overview,
    pub stages: Vec<Stage>,
    pub details: BTreeMap<String, Detail>,
}

impl Series {
    pub fn empty(scope: Scope) -> Self {
        Self {
            schema_version: SERIES_VERSION,
            scope,
            updated_at: 0,
            fetch_count: 0,
            overview: Overview {
                today_str: String::new(),
                today_stats: Vec::new(),
                trend_data: Vec::new(),
            },
            stages: Vec::new(),
            details: BTreeMap::new(),
        }
    }

    /// 把一次采集并入序列：点位按日期去重、较新覆盖；作品取并集。
    pub fn merge(
        &mut self,
        fetched_at: u64,
        overview: Overview,
        stages: Vec<Stage>,
        details: BTreeMap<String, Detail>,
    ) {
        self.schema_version = SERIES_VERSION;
        self.updated_at = fetched_at;
        self.fetch_count = self.fetch_count.saturating_add(1);
        self.overview.today_str = overview.today_str;
        self.overview.today_stats = overview.today_stats;
        merge_trends(&mut self.overview.trend_data, &overview.trend_data);
        // The API returns stages in its selected sort order. Keep that order for
        // the current response, then retain previously archived stages it omitted.
        let previous_order: Vec<String> = self
            .stages
            .iter()
            .map(|stage| stage.base_info.stage_id.clone())
            .collect();
        let mut previous: BTreeMap<String, Stage> = std::mem::take(&mut self.stages)
            .into_iter()
            .map(|stage| (stage.base_info.stage_id.clone(), stage))
            .collect();
        let mut seen = BTreeSet::new();
        let mut merged = Vec::with_capacity(stages.len() + previous.len());
        for stage in stages {
            let id = stage.base_info.stage_id.clone();
            if seen.insert(id.clone()) {
                previous.remove(&id);
                merged.push(stage);
            }
        }
        for id in previous_order {
            if let Some(stage) = previous.remove(&id) {
                merged.push(stage);
            }
        }
        self.stages = merged;
        for (id, incoming) in details {
            match self.details.get_mut(&id) {
                Some(slot) => {
                    slot.today_str = incoming.today_str;
                    slot.stage_info = incoming.stage_info;
                    slot.today_stats = incoming.today_stats;
                    slot.metric_groups = incoming.metric_groups;
                    slot.comment_module_info = incoming.comment_module_info;
                    merge_trends(&mut slot.trend_data, &incoming.trend_data);
                }
                None => {
                    self.details.insert(id, incoming);
                }
            }
        }
    }
}

/// 合并趋势：按 (metric_type, date) 去重、较新覆盖；变化量按合并后的整条序列重算。
fn merge_trends(target: &mut Vec<Trend>, incoming: &[Trend]) {
    for trend in incoming {
        match target
            .iter_mut()
            .find(|t| t.metric_type == trend.metric_type)
        {
            Some(slot) => {
                slot.value_type = trend.value_type.clone();
                slot.calculate_type = trend.calculate_type.clone();
                slot.normalized_stats_7d = trend.normalized_stats_7d.clone();
                slot.multi_group_dist = trend.multi_group_dist.clone();
                merge_points(&mut slot.daily_stats, &trend.daily_stats);
                finalize(slot);
            }
            None => {
                let mut fresh = trend.clone();
                finalize(&mut fresh);
                target.push(fresh);
            }
        }
    }
}

fn merge_points(target: &mut Vec<DailyStat>, incoming: &[DailyStat]) {
    let mut merged: BTreeMap<String, DailyStat> = target
        .iter()
        .cloned()
        .map(|p| (p.date.clone(), p))
        .collect();
    for point in incoming {
        merged.insert(point.date.clone(), point.clone());
    }
    *target = merged.into_values().collect();
}

/// 用合并后的序列重算 7/30 日变化；点数不够时留空（界面显示 —，不按 0 计算）。
fn finalize(trend: &mut Trend) {
    trend.daily_stats.sort_by(|a, b| a.date.cmp(&b.date));
    (trend.delta_7_day, trend.delta_7_day_invalid) = change(&trend.daily_stats, 7);
    (trend.delta_30_day, trend.delta_30_day_invalid) = change(&trend.daily_stats, 30);
}

fn change(points: &[DailyStat], days: usize) -> (String, bool) {
    let last = points.len().checked_sub(1);
    let previous = last.and_then(|index| index.checked_sub(days));
    let (Some(last), Some(previous)) = (last, previous) else {
        return (String::new(), false);
    };
    if points[last].cur_invalid || points[previous].cur_invalid {
        return (String::new(), true);
    }
    match (
        points[last].cur.parse::<f64>(),
        points[previous].cur.parse::<f64>(),
    ) {
        (Ok(cur), Ok(previous)) => (number(cur - previous), false),
        _ => (String::new(), false),
    }
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::{DailyStat, Trend, finalize};

    fn points(count: usize) -> Vec<DailyStat> {
        (0..count)
            .map(|day| DailyStat {
                date: format!("2026-01-{:02}", day + 1),
                cur: (100 + day).to_string(),
                delta: String::new(),
                value_type: "METRIC_VALUE_TYPE_NUMBER".into(),
                cur_invalid: false,
            })
            .collect()
    }

    #[test]
    fn finalize_marks_invalid_comparison_endpoints() {
        let mut seven_day = Trend {
            daily_stats: points(8),
            ..Trend::default()
        };
        seven_day.daily_stats[0].cur_invalid = true;
        finalize(&mut seven_day);
        assert!(seven_day.delta_7_day.is_empty());
        assert!(seven_day.delta_7_day_invalid);

        let mut thirty_day = Trend {
            daily_stats: points(31),
            ..Trend::default()
        };
        thirty_day.daily_stats[0].cur_invalid = true;
        finalize(&mut thirty_day);
        assert!(thirty_day.delta_30_day.is_empty());
        assert!(thirty_day.delta_30_day_invalid);
        assert!(!thirty_day.delta_7_day_invalid);
    }

    #[test]
    fn finalize_computes_deltas_for_valid_endpoints() {
        let mut trend = Trend {
            daily_stats: points(31),
            ..Trend::default()
        };
        finalize(&mut trend);
        assert_eq!(trend.delta_7_day, "7");
        assert_eq!(trend.delta_30_day, "30");
        assert!(!trend.delta_7_day_invalid);
        assert!(!trend.delta_30_day_invalid);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagePage {
    pub stage_list: Vec<Stage>,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct DistributionRange {
    pub min: String,
    pub max: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct DistributionConfig {
    pub group_index: u32,
    pub name_mi18n_key: String,
    pub ranges: Vec<DistributionRange>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Bucket {
    pub min: String,
    pub max: String,
    pub count: String,
    pub percent: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct DistributionDay {
    pub date: String,
    pub buckets: Vec<Bucket>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct DistributionData {
    pub daily_items: Vec<DistributionDay>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Distribution {
    pub group_configs: Vec<DistributionConfig>,
    pub group_data: BTreeMap<String, DistributionData>,
    pub recommended_group_index: u32,
}
