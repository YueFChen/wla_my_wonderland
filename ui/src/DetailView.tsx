import { useState } from 'react'
import type { Detail } from './types.generated'

import { Trends } from './Charts'
import { Stats } from './Stats'
import { formatTime, statusClass, statusLabel } from './display'
import { t } from './i18n'
import { metricLabel } from './i18n/metric'
import { CoverImage } from './CoverImage'

/** 单个关卡详情：封面信息与指标分组并排，后接趋势与评价。 */
export function DetailView({ detail }: { detail: Detail }) {
  const [group, setGroup] = useState('all')
  const active = group === 'all' ? undefined : detail.metric_groups.find((item) => item.group_type === group)
  const selectedGroup = active ? group : 'all'
  const trends = active
    ? detail.trend_data.filter((item) => active.metric_types.includes(item.metric_type))
    : detail.trend_data
  const stats = active
    ? detail.today_stats.filter((item) => active.metric_types.includes(item.metric_type))
    : detail.today_stats
  const info = detail.stage_info
  const cover = info.cover_url?.startsWith('https://') ? info.cover_url : ''
  const comments = detail.comment_module_info

  return (
    <>
      <div className="mw-detail-overview">
        <section className="mw-hero">
          <div className="mw-hero-media">
            <CoverImage src={cover} alt="" />
          </div>
          <div className="mw-hero-body">
            <span className={`mw-status ${statusClass(info.game_list_status)}`}>
              {statusLabel(info.game_list_status)}
            </span>
            <h2>{info.stage_name}</h2>
            <div className="mw-hero-meta">
              <span>#{info.stage_id}</span>
              <span>
                {t('detail.latestOnline')} · {formatTime(info.latest_online_time)}
              </span>
            </div>
          </div>
        </section>
        <div className="mw-detail-metrics">
          <div className="mw-groups" data-label={t('detail.metricGroup')}>
            <div className="mw-range">
              <button type="button" aria-pressed={selectedGroup === 'all'} onClick={() => setGroup('all')}>
                {t('detail.allGroups')}
              </button>
              {detail.metric_groups.map((item) => (
                <button
                  type="button"
                  aria-pressed={selectedGroup === item.group_type}
                  key={item.group_type}
                  onClick={() => setGroup(item.group_type)}
                >
                  {metricLabel(item.group_type)}
                </button>
              ))}
            </div>
          </div>
          <Stats key={selectedGroup} stats={stats} className="mw-detail-stats" />
        </div>
      </div>

      <Trends trends={trends} detail />

      <section>
        <div className="mw-section-head">
          <h2>{t('detail.comments')}</h2>
          <span className="mw-count">{comments.total_comment_num || '0'}</span>
        </div>
        {comments.comment_list.length ? (
          <div className="mw-comment-list">
            {comments.comment_list.map((comment) => (
              <article className="mw-comment" key={comment.comment_id}>
                <header>
                  <strong>{comment.nickname}</strong>
                  <span className={`mw-pill ${comment.is_recommend ? 'mw-pill--up' : 'mw-pill--down'}`}>
                    {comment.is_recommend ? t('detail.recommend') : t('detail.notRecommend')}
                  </span>
                  <time>{formatTime(comment.comment_unix)}</time>
                </header>
                <p>{comment.content}</p>
              </article>
            ))}
          </div>
        ) : (
          <div className="mw-empty">{t('detail.noComments')}</div>
        )}
      </section>
    </>
  )
}
