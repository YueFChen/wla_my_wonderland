import { createContext, useContext, useId, useEffect, useState, type MouseEvent as ReactMouseEvent, type ReactNode } from 'react'
import { Drawer } from '@wonderland/ui'
import type { Distribution, Trend } from './types.generated'

import { formatMetric, numeric, trendPoints } from './metrics'
import { t } from './i18n'
import { metricLabel } from './i18n/metric'

export const WonderlandPanelContext = createContext<{ active: string | null; select: (id: string | null) => void } | null>(null)

const DISTRIBUTION_COLORS = ['#7663b4', '#9b7bd1', '#b6a0ed', '#d3c6f1', '#eedbfc']

/** 趋势区：按 7 / 30 天（或本地留存全期）展示折线与时长分布。 */
export function Trends({ trends, detail = false }: { trends: Trend[]; detail?: boolean }) {
  // 默认看官方滚动窗口；本地留存更久时可切到全期。
  const [days, setDays] = useState(30)
  const ranges: [number, string][] = [
    [7, t('chart.range7')],
    [30, t('chart.range30')],
    [0, t('chart.rangeAll')],
  ]

  // 分布图紧跟对应的趋势图，和其余图表一起进入响应式网格。
  const items: ReactNode[] = []
  for (const trend of trends) {
    items.push(<Chart key={trend.metric_type} trend={trend} days={days} />)
    if (trend.multi_group_dist) {
      items.push(
        <DistributionChart key={`${trend.metric_type}-dist`} data={trend.multi_group_dist} days={days} />,
      )
    }
  }

  return (
    <>
      <div className="mw-chart-heading">
        <h2>{t('chart.title')}</h2>
        <div className="mw-range">
          {ranges.map(([value, label]) => (
            <button type="button" key={value} aria-pressed={days === value} onClick={() => setDays(value)}>
              {label}
            </button>
          ))}
        </div>
      </div>
      {trends.length === 0 ? (
        <div className="mw-empty">{t('chart.noData')}</div>
      ) : (
        <div className={`mw-charts${detail ? ' mw-charts--detail' : ''}`}>{items}</div>
      )}
    </>
  )
}

function Chart({ trend, days }: { trend: Trend; days: number }) {
  const [hover, setHover] = useState<{ index: number; left: number; top: number; below: boolean } | null>(null)
  const points = trendPoints(trend, days)
  const values = points.map((point) =>
    point.cur_invalid ? null : numeric(point.cur, point.value_type, trend.metric_type),
  )
  const valid = values.filter((value): value is number => value !== null)
  const min = valid.length ? Math.min(...valid) : 0
  const max = valid.length ? Math.max(...valid) : 0
  const span = max - min || Math.max(Math.abs(max) * 0.1, 1)
  const xy = (index: number, value: number): [number, number] => [
    48 + (index / Math.max(1, points.length - 1)) * 470,
    150 - ((value - min) / span) * 108,
  ]
  useEffect(() => setHover(null), [days, points.length, trend.metric_type])

  const onPlotMove = (event: ReactMouseEvent<SVGSVGElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0) return
    const pointerX = ((event.clientX - rect.left) / rect.width) * 560
    const plotRect = event.currentTarget.parentElement?.getBoundingClientRect() ?? rect
    let nearestIndex = -1
    let nearestDistance = Number.POSITIVE_INFINITY
    for (let index = 0; index < values.length; index++) {
      const value = values[index]
      if (value === null) continue
      const distance = Math.abs(xy(index, value)[0] - pointerX)
      if (distance < nearestDistance) {
        nearestIndex = index
        nearestDistance = distance
      }
    }
    if (nearestIndex < 0) return
    const index = nearestIndex
    const value = values[index]
    if (value === null) return
    const [x, y] = xy(index, value)
    const halfTooltip = Math.min(92, plotRect.width / 2 - 8)
    const pointLeft = rect.left - plotRect.left + (x / 560) * rect.width
    const left = Math.max(halfTooltip, Math.min(plotRect.width - halfTooltip, pointLeft))
    const top = rect.top - plotRect.top + (y / 190) * rect.height
    setHover((current) => current?.index === index
      ? current
      : { index, left, top, below: top < 58 })
  }
  const hoveredValue = hover && hover.index < values.length ? values[hover.index] : null
  const hoveredPoint = hover && hoveredValue !== null ? points[hover.index] : null
  const hoveredCoords = hover && hoveredValue !== null ? xy(hover.index, hoveredValue) : null

  let path = ''
  let gap = true
  values.forEach((value, index) => {
    if (value === null) {
      gap = true
      return
    }
    const [x, y] = xy(index, value)
    path += `${gap ? 'M' : 'L'}${x},${y} `
    gap = false
  })

  const change7 = formatMetric(
    trend.delta_7_day,
    trend.value_type,
    trend.metric_type,
    trend.delta_7_day_invalid,
    trend.calculate_type,
  )
  const change30 = formatMetric(
    trend.delta_30_day,
    trend.value_type,
    trend.metric_type,
    trend.delta_30_day_invalid,
    trend.calculate_type,
  )

  return (
    <article className="mw-panel mw-chart">
      <h3>{metricLabel(trend.metric_type)}</h3>
      <p className="mw-chart-summary">
        <span>
          {t('chart.delta7')}
          <b>{change7}</b>
        </span>
        <span>
          {t('chart.delta30')}
          <b>{change30}</b>
        </span>
      </p>
      {!valid.length ? (
        <p className="mw-chart-empty">{t('chart.noTrend')}</p>
      ) : (
        <div className="mw-chart-plot">
          <svg
            viewBox="0 0 560 190"
            role="img"
            aria-label={t('chart.aria', { metric: metricLabel(trend.metric_type) })}
            onMouseMove={onPlotMove}
            onMouseLeave={() => setHover(null)}
          >
            {[42, 96, 150].map((y) => (
              <line key={y} x1="48" x2="518" y1={y} y2={y} stroke="currentColor" opacity=".1" />
            ))}
            <text x="6" y="45">{max.toLocaleString('zh-CN', { maximumFractionDigits: 1 })}</text>
            <text x="6" y="154">{min.toLocaleString('zh-CN', { maximumFractionDigits: 1 })}</text>
            <path d={path} fill="none" stroke="currentColor" strokeWidth="2.5" />
            {values.map((value, index) => {
              if (value === null) return null
              const [cx, cy] = xy(index, value)
              return <circle key={points[index].date} cx={cx} cy={cy} r="3" fill="currentColor" />
            })}
            {hoveredCoords && (
              <g aria-hidden="true" pointerEvents="none">
                <line
                  x1={hoveredCoords[0]}
                  x2={hoveredCoords[0]}
                  y1="42"
                  y2="150"
                  stroke="currentColor"
                  strokeDasharray="4 4"
                  opacity=".45"
                />
                <circle
                  cx={hoveredCoords[0]}
                  cy={hoveredCoords[1]}
                  r="6"
                  fill="currentColor"
                  stroke="var(--app-glass-subtle)"
                  strokeWidth="3"
                />
              </g>
            )}
            <text x="48" y="181">{points[0]?.date}</text>
            <text x="518" y="181" textAnchor="end">{points.at(-1)?.date}</text>
          </svg>
          {hover && hoveredValue !== null && hoveredPoint && (
            <div
              className="mw-chart-tooltip"
              role="tooltip"
              style={{
                left: `${hover.left}px`,
                top: `${hover.below ? hover.top + 8 : hover.top - 8}px`,
                transform: `translate(-50%, ${hover.below ? '0' : '-100%'})`,
              }}
            >
              <strong>
                {formatMetric(hoveredPoint.cur, hoveredPoint.value_type, trend.metric_type, false, trend.calculate_type)}
              </strong>
              <span>{hoveredPoint.date}</span>
            </div>
          )}
        </div>
      )}
      <DataPanel title={metricLabel(trend.metric_type)} label={t('chart.details')}>
        <div className="mw-point-table">
          <table>
            <thead>
              <tr>
                <th>{t('chart.pointsDate')}</th>
                <th>{t('chart.pointsValue')}</th>
              </tr>
            </thead>
            <tbody>
              {points.map((point) => (
                <tr key={point.date}>
                  <td>{point.date}</td>
                  <td>{formatMetric(point.cur, point.value_type, trend.metric_type, point.cur_invalid, trend.calculate_type)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </DataPanel>
    </article>
  )
}

/** 游玩时长分桶分布；官方以百分比的整数形式返回，这里还原为占比。 */
export function DistributionChart({ data, days }: { data: Distribution; days: number }) {
  const [selected, setSelected] = useState(data.recommended_group_index)
  const config = data.group_configs.find((group) => group.group_index === selected) ?? data.group_configs[0]
  if (!config) return null

  const rows = (data.group_data[String(config.group_index)]?.daily_items ?? []).slice(-days)
  const label = (min: string, max: string) =>
    max === '-1' ? t('stats.minutesAbove', { min }) : t('stats.minutesRange', { min, max })

  return (
    <article className="mw-panel mw-chart mw-distribution">
      <div className="mw-chart-heading">
        <h3>{t('stats.distributionTitle')}</h3>
        <select
          aria-label={t('stats.distributionAria')}
          value={config.group_index}
          onChange={(event) => setSelected(Number(event.target.value))}
        >
          {data.group_configs.map((group, index) => (
            <option key={group.group_index} value={group.group_index}>
              {t('stats.distributionRange', { index: index + 1 })}
              {group.group_index === data.recommended_group_index
                ? ` · ${t('stats.distributionRecommended')}`
                : ''}
            </option>
          ))}
        </select>
      </div>
      <div className="mw-legend">
        {config.ranges.map((range, index) => (
          <span key={index}>
            <i style={{ background: DISTRIBUTION_COLORS[index % DISTRIBUTION_COLORS.length] }} />
            {label(range.min, range.max)}
          </span>
        ))}
      </div>
      <div className="mw-distribution-scroll">
        <div className="mw-bars">
          {rows.map((row) => (
            <div key={row.date} className="mw-bar-column">
              <div className="mw-bar">
                {row.buckets.map((bucket, index) => (
                  <div
                    key={index}
                    style={{
                      height: `${Math.min(100, bucket.percent / 100)}%`,
                      background: DISTRIBUTION_COLORS[index % DISTRIBUTION_COLORS.length],
                    }}
                    title={`${row.date} · ${label(bucket.min, bucket.max)} · ${bucket.percent / 100}% · ${bucket.count} ${t('chart.pointsCount')}`}
                  />
                ))}
              </div>
              <small>{row.date.slice(5)}</small>
            </div>
          ))}
        </div>
      </div>
      <DataPanel title={t('stats.distributionTitle')} label={t('stats.distributionDetail')}>
        <div className="mw-point-table">
          <table>
            <thead>
              <tr>
                <th>{t('chart.pointsDate')}</th>
                <th>{t('chart.pointsRange')}</th>
                <th>{t('chart.pointsPercent')}</th>
                <th>{t('chart.pointsCount')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.flatMap((row) =>
                row.buckets.map((bucket, index) => (
                  <tr key={`${row.date}-${index}`}>
                    <td>{row.date}</td>
                    <td>{label(bucket.min, bucket.max)}</td>
                    <td>{bucket.percent / 100}%</td>
                    <td>{bucket.count}</td>
                  </tr>
                )),
              )}
            </tbody>
          </table>
        </div>
      </DataPanel>
    </article>
  )
}

function DataPanel({ title, label, children }: { title: string; label: string; children: ReactNode }) {
  const id = useId()
  const panels = useContext(WonderlandPanelContext)
  const [localOpen, setLocalOpen] = useState(false)
  const open = panels ? panels.active === id : localOpen
  const show = () => panels ? panels.select(id) : setLocalOpen(true)
  const close = () => panels ? panels.select(null) : setLocalOpen(false)
  return <>
    <button className="mw-data-trigger" type="button" aria-expanded={open} onClick={show}>{label}</button>
    <Drawer open={open} title={title} onClose={close} closeLabel={t('common.close')} size="md" bodyClassName="mw-data-body">{children}</Drawer>
  </>
}
