/**
 * 插件界面文案资源表。
 *
 * 键 = `<域>.<语义>`，点分、段内 camelCase；域取页面 / 视图名：
 * `works`（关卡列表页）、`detail`（关卡详情）、`stats`（指标与时长分布）、
 * `chart`（趋势图）、`common`（跨视图复用）、`metric`（数值单位）。
 * 官方指标代号 → 显示名的映射见 `metric.ts`。
 *
 * 带参数的条目用 `{name}` 占位符，调用处传 `t(key, { name })`。
 */
export const zh = {
  'history.import': '导入采集历史',
  'history.export': '导出采集历史',
  'history.merge': '选择文件并合并',
  'history.confirm': '按账号与角色合并历史，重合记录采用较新采集结果。备份不包含账号登录信息，账号请重新登录。也可选择旧 series-v2 目录中的单个 JSON 文件。',
  'history.imported': '新增 {created} 个角色，合并 {merged} 个，未变更 {unchanged} 个。',
  'history.saved': '已保存：',
  'history.ready': '文件已就绪，请在 Core 中下载：',
  'account.title': '游戏账号',
  'account.platform': '账号来源',
  'account.manage': '账号管理',
  'account.login': '登录账号',
  'account.cancelLogin': '取消登录',
  'account.logging': '等待完成登录',
  'account.sync': '同步资料',
  'account.setDefault': '设为默认',
  'account.default': '默认',
  'account.forget': '遗忘账号',
  'account.forgetTitle': '遗忘账号',
  'account.forgetMessage': '移除该账号和保存的登录凭据，已采集的奇域记录保留。',
  'account.passport': '通行证',
  'account.creator': '奇匠',
  'account.close': '关闭',
  'account.empty': '暂无已保存账号',
  'account.remoteEmpty': '暂无可用账号，请在宿主的“我的奇域”中登录。',
  'account.windowClosed': '登录窗口已关闭',
  'account.timeout': '等待登录超时',
  'account.cancelled': '已取消登录',
  'account.failed': '登录未完成，请重试',
  'account.retry': '重试',
  /** 关卡列表页（index.tsx / WorksView.tsx）。 */
  'works.title': '我的奇域',
  'works.loading': '正在读取本地数据…',
  'works.emptyLocal': '暂无本地关卡数据。',
  'works.empty': '该角色暂无关卡。',
  'works.emptyOnline': '暂无在线关卡。',
  'works.emptyOffline': '暂无离线关卡。',
  'works.emptyDeleted': '暂无已删除关卡。',
  'works.noAccount': '请在账号管理中登录并同步角色。',
  'works.noRoles': '暂无可选角色',
  'works.account': '账号 / 角色',
  'works.listHeading': '关卡列表',
  'works.updatedAt': '数据更新于',
  'works.updating': '正在更新…',
  'works.refresh': '刷新数据',
  'works.retry': '重试采集',
  'works.collecting': '正在采集总览、关卡与详情…',
  /** 关卡状态筛选。 */
  'works.filterAria': '关卡状态筛选',
  'works.filterAll': '全部',
  'works.filterOnline': '在线',
  'works.filterOffline': '离线',
  'works.filterDeleted': '已删除',
  'works.totalStages': '总关卡数',
  'works.onlineStages': '在线关卡',
  'works.totalHot': '总热度',
  'works.avgRate': '平均推荐率',
  'works.back': '返回关卡列表',
  'works.viewDetail': '查看详情',
  'works.stageId': '关卡 ID',
  'works.hotScore': '热度',
  'works.goodRate': '推荐率',
  'works.avgTime': '平均时长',
  /** 官方只给滚动窗口，本地按日期去重后长期留存。 */
  'works.archivePolicy': '数据按日期去重后留存于本机，官方窗口之外的历史仍可查看。',
  'works.partial': '单条详情失败不会丢弃已获取的数据；完成后可手动重试。',
  'works.previous': '上一页',
  'works.next': '下一页',

  /** 关卡详情（DetailView.tsx）。 */
  'detail.latestOnline': '最近上线',
  'detail.metricGroup': '指标分组',
  'detail.allGroups': '全部指标',
  'detail.comments': '关卡评价',
  'detail.commentsHint': '展示统计接口返回的评价样本。',
  'detail.recommend': '推荐',
  'detail.notRecommend': '不推荐',
  'detail.noComments': '暂无评价样本',

  /** 指标卡片与时长分布（Stats.tsx / Charts.tsx 分布图）。 */
  'stats.changedVsYesterday': '较昨日',
  'stats.distributionTitle': '游玩时长分布',
  'stats.distributionAria': '时长分布区间',
  'stats.distributionDetail': '查看分布明细',
  'stats.distributionRecommended': '推荐',
  /** 时长分布区间名，参数为区间序号。 */
  'stats.distributionRange': '区间 {index}',
  'stats.minutesAbove': '{min} 分钟以上',
  'stats.minutesRange': '{min}–{max} 分钟',

  /** 趋势图（Charts.tsx）。 */
  'chart.title': '趋势分析',
  'chart.range7': '近 7 天',
  'chart.range30': '近 30 天',
  'chart.rangeAll': '全部历史',
  'chart.noTrend': '此指标暂无趋势数据',
  'chart.noData': '暂无趋势数据',
  'chart.delta7': '7 日变化',
  'chart.delta30': '30 日变化',
  /** 图表无障碍描述，参数为指标名。 */
  'chart.aria': '{metric}趋势图',
  'chart.details': '查看数据明细',
  /** 数据明细表列头（趋势表与分布表共用）。 */
  'chart.pointsDate': '日期',
  'chart.pointsValue': '数值',
  'chart.pointsRange': '时长区间',
  'chart.pointsPercent': '占比',
  'chart.pointsCount': '次数',

  /** 跨视图复用。 */
  'common.worksUnit': '{count} 个关卡',
  'common.statusOnline': '已上线',
  'common.statusOffline': '已下线',
  'common.statusDeleted': '已删除',
  'common.unknown': '未知状态',
  'common.failed': '操作失败',
  'common.close': '关闭',

  /** 数值单位（metrics.ts）。 */
  'metric.hours': '{hours}小时',
  'metric.duration': '{minutes}分 {seconds}秒',
} as const
