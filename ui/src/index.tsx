import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { Scope, Series, AccountSnapshot } from './types.generated'

import { Trends } from './Charts'
import { DetailView } from './DetailView'
import { WorksGrid } from './WorksView'
import { errorMessage, formatTime } from './display'
import { t, type MessageKey } from './i18n'
import { summarizeStages } from './summary'
import { needsCollect } from './refresh'
import { roleLabel } from './accounts-model'
import './style.css'

/** 插件页面接口；由 main.tsx 通过 UI Bridge 适配 Core 与插件后端。 */
export interface WonderlandApi {
  /** Read this plugin's saved account metadata. */
  accountSnapshot(): Promise<AccountSnapshot>
  /** 采集一次并并入本地序列，返回合并后的序列。 */
  collect(scope: Scope): Promise<Series>
  /** 只读本地序列；从未采集过时为 null。 */
  series(scope: Scope): Promise<Series | null>
  rememberScope?(key: string): Promise<void>
}

interface Props {
  api: WonderlandApi
  snapshot: AccountSnapshot | null
  initialSelection?: string
  activeRevision?: number
  remote?: boolean
  onManageAccounts?: () => void
}

type Filter = 'all' | 'online' | 'offline' | 'deleted'

// 存 key 而不是取好的文案：这张表是模块级常量，取值留到渲染时才发生。
const FILTERS: { value: Filter; labelKey: MessageKey; emptyKey: MessageKey; status?: string }[] = [
  { value: 'all', labelKey: 'works.filterAll', emptyKey: 'works.empty' },
  {
    value: 'online',
    labelKey: 'works.filterOnline',
    emptyKey: 'works.emptyOnline',
    status: 'GAME_LIST_STATUS_ONLINE',
  },
  {
    value: 'offline',
    labelKey: 'works.filterOffline',
    emptyKey: 'works.emptyOffline',
    status: 'GAME_LIST_STATUS_OFFLINE',
  },
  {
    value: 'deleted',
    labelKey: 'works.filterDeleted',
    emptyKey: 'works.emptyDeleted',
    status: 'GAME_LIST_STATUS_DELETED',
  },
]

const scopeKey = (scope: Scope) => `${scope.account_key}/${scope.region}/${scope.uid}`
const emptyScope: Scope = { account_key: '', uid: '', region: '' }

export function MyWonderlandPage({ api, snapshot, initialSelection = '', activeRevision = 0, remote = false, onManageAccounts }: Props) {
  const [series, setSeries] = useState<Series | null>(null)
  const [selected, setSelected] = useState(initialSelection)
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [filter, setFilter] = useState<Filter>('online')
  const [stageId, setStageId] = useState<string | null>(null)
  const listScroll = useRef(0)
  const restoreListScroll = useRef(false)
  // 账号/角色切换后丢弃在途响应，避免旧数据覆盖新选择。
  const generation = useRef(0)
  const mounted = useRef(true)

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
      generation.current++
    }
  }, [])

  // 可选范围 = 当前账号绑定的游戏角色（采集要求角色属于该账号）。
  const scopes = useMemo(() => {
    const list: { scope: Scope; label: string; refreshHour: number | null }[] = []
    for (const account of snapshot?.accounts ?? []) {
      for (const role of account.game_roles) {
        list.push({
          scope: { account_key: account.account_key, uid: role.uid, region: role.region },
          label: roleLabel(account, role),
          refreshHour: role.data_refresh_hour ?? null,
        })
      }
    }
    return list
  }, [snapshot])

  const current = scopes.find((item) => item.scope.account_key === snapshot?.current_account_key)
  const chosen = scopes.find((item) => scopeKey(item.scope) === selected) ?? current ?? scopes[0]
  const key = scopeKey(chosen?.scope ?? emptyScope)
  const scope = chosen?.scope

  useEffect(() => {
    const id = ++generation.current
    setSeries(null)
    setError('')
    setStageId(null)
    setFilter('online')
    setBusy(false)
    if (!scope) {
      setLoading(false)
      return
    }
    setLoading(true)
    void (async () => {
      try {
        // 先用本地序列秒开，再按「当天是否已抓过」决定要不要自动更新。
        const cached = await api.series(scope)
        if (generation.current !== id || !mounted.current) return
        setSeries(cached)
        setLoading(false)
        if (cached && !needsCollect(cached.updated_at, new Date(), chosen?.refreshHour)) return
        setBusy(true)
        const fresh = await api.collect(scope)
        if (generation.current !== id || !mounted.current) return
        setSeries(fresh)
        setError('')
      } catch (cause) {
        if (generation.current === id && mounted.current) {
          setError(errorMessage(cause))
          // Detail failures are reported after the successful parts were saved.
          // Reload that partial result so a first collection still shows its data.
          try {
            const saved = await api.series(scope)
            if (generation.current === id && mounted.current && saved) setSeries(saved)
          } catch {
            // Keep the original collection error visible.
          }
        }
      } finally {
        if (generation.current === id && mounted.current) {
          setLoading(false)
          setBusy(false)
        }
      }
    })()
    return () => {
      generation.current++
    }
    // scope 每次账号刷新都会重建，用稳定的 key 代表其身份。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, key])

  const refresh = () => {
    if (!scope || busy || loading) return
    const refreshScope = scope
    const id = ++generation.current
    setError('')
    setBusy(true)
    void (async () => {
      try {
        const fresh = await api.collect(refreshScope)
        if (generation.current !== id || !mounted.current) return
        setSeries(fresh)
      } catch (cause) {
        if (generation.current !== id || !mounted.current) return
        setError(errorMessage(cause))
        // collect may report a partial result after persisting it locally.
        try {
          const saved = await api.series(refreshScope)
          if (generation.current === id && mounted.current && saved) setSeries(saved)
        } catch {
          // Keep the previous in-memory data and the collection error.
        }
      } finally {
        if (generation.current === id && mounted.current) setBusy(false)
      }
    })()
  }

  const recheck = useRef(() => {})
  const lastActivation = useRef({ key, revision: activeRevision })
  recheck.current = () => {
    if (scope && !loading && !busy && (!series || needsCollect(series.updated_at, new Date(), chosen?.refreshHour))) refresh()
  }
  useEffect(() => {
    const previous = lastActivation.current
    lastActivation.current = { key, revision: activeRevision }
    if (previous.key === key && previous.revision !== activeRevision) recheck.current()
  }, [key, activeRevision])

  const active = FILTERS.find((item) => item.value === filter) ?? FILTERS[0]
  const stages = useMemo(() => {
    const list = series?.stages ?? []
    return active.status ? list.filter((stage) => stage.base_info.game_list_status === active.status) : list
  }, [series, active.status])

  // 概览只汇总有效值；有效的 0 参与计算，无效值不按 0 处理。
  const summary = useMemo(() => summarizeStages(series?.stages ?? []), [series])

  const detail = stageId ? series?.details[stageId] : undefined

  useLayoutEffect(() => {
    if (detail) window.scrollTo({ top: 0, behavior: 'instant' })
    else if (restoreListScroll.current) {
      window.scrollTo({ top: listScroll.current, behavior: 'instant' })
      restoreListScroll.current = false
    }
  }, [detail])

  return (
    <section className="mw" aria-label={t('works.title')}>
      <div className="mw-toolbar">
        {detail && (
          <button type="button" className="mw-back" onClick={() => {
            restoreListScroll.current = true
            setStageId(null)
          }}>
            ← {t('works.back')}
          </button>
        )}
        <div className="mw-toolbar-end">
          {onManageAccounts && <button type="button" className="mw-refresh" onClick={onManageAccounts}>{t('account.manage')}</button>}
          <button
            type="button"
            className="mw-refresh"
            disabled={busy || loading || !scope}
            onClick={refresh}
          >
            {t(error ? 'works.retry' : 'works.refresh')}
          </button>
          <label className="mw-role-picker">
            <span>{t('works.account')}</span>
            <select
              aria-label={t('works.account')}
              value={key}
              disabled={!snapshot || busy || scopes.length === 0}
              onChange={(event) => {
                generation.current++
                setSelected(event.target.value)
                void api.rememberScope?.(event.target.value).catch(() => undefined)
              }}
            >
              {scopes.length === 0 ? (
                <option value={key}>{t(snapshot ? 'works.noRoles' : 'works.loading')}</option>
              ) : (
                scopes.map((item) => (
                  <option key={scopeKey(item.scope)} value={scopeKey(item.scope)}>
                    {item.label}
                  </option>
                ))
              )}
            </select>
          </label>
          <span className="mw-toolbar-meta">
            {!snapshot
              ? t('works.loading')
              : !scope
                ? t('works.noRoles')
                : busy
                  ? t('works.updating')
                  : series
                    ? `${t('works.updatedAt')} · ${formatTime(series.updated_at)}`
                    : t('works.archivePolicy')}
          </span>
        </div>
      </div>

      {error && (
        <div className="mw-error" role="alert">
          <span>{error}</span>
        </div>
      )}
      {busy && (
        <div className="mw-progress" role="status">
          <span />
          {t('works.collecting')}
          <small>{t('works.partial')}</small>
        </div>
      )}

      {!snapshot ? (
        <div className="mw-empty" role="status">
          {t('works.loading')}
        </div>
      ) : !scope ? (
        <div className="mw-empty" role="status">
          {t(remote ? 'account.remoteEmpty' : 'works.noAccount')}
        </div>
      ) : loading ? (
        <div className="mw-empty" role="status">
          {t('works.loading')}
        </div>
      ) : !series ? (
        <div className="mw-empty">{t('works.emptyLocal')}</div>
      ) : detail ? (
        <div className="mw-view" key={`detail-${stageId}`}>
          <DetailView detail={detail} />
        </div>
      ) : (
        <div className="mw-view" key="works-list">
          <div className="mw-stats mw-overview-stats">
            <article className="mw-stat">
              <span>{t('works.totalStages')}</span>
              <strong>{summary.total}</strong>
            </article>
            <article className="mw-stat">
              <span>{t('works.onlineStages')}</span>
              <strong>{summary.online}</strong>
            </article>
            <article className="mw-stat">
              <span>{t('works.totalHot')}</span>
              <strong>{summary.hot === null ? '—' : summary.hot.toLocaleString('zh-CN')}</strong>
            </article>
            <article className="mw-stat">
              <span>{t('works.avgRate')}</span>
              <strong>{summary.rate === null ? '—' : `${summary.rate.toFixed(2)}%`}</strong>
            </article>
          </div>

          <div className="mw-list-head">
            <div className="mw-list-title">
              <h2>{t('works.listHeading')}</h2>
              <span>{t('common.worksUnit', { count: stages.length })}</span>
            </div>
            <nav className="mw-filters" aria-label={t('works.filterAria')}>
              {FILTERS.map((item) => (
                <button
                  type="button"
                  key={item.value}
                  aria-pressed={filter === item.value}
                  onClick={() => setFilter(item.value)}
                >
                  {t(item.labelKey)}
                </button>
              ))}
            </nav>
          </div>

          <WorksGrid
            key={filter}
            stages={stages}
            details={series.details}
            empty={t(active.emptyKey)}
            onOpen={(id) => {
              listScroll.current = window.scrollY
              setStageId(id)
            }}
          />

          {series.overview.trend_data.length > 0 && <Trends trends={series.overview.trend_data} />}
        </div>
      )}
    </section>
  )
}
