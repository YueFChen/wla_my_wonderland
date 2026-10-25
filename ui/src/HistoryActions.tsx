import { useEffect, useRef, useState } from 'react'
import { confirmDialog } from '@wonderland/ui'
import type { FileResult, PluginHostClient } from '@wonderland/plugin-ui-sdk'
import type { HistoryImportReport } from './types.generated'
import { t } from './i18n'

export function HistoryActions({ host, onImported }: { host: PluginHostClient; onImported: () => void }) {
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')
  const running = useRef(false)
  const live = useRef(false)
  useEffect(() => { live.current = true; return () => { live.current = false } }, [])

  async function run(operation: 'import' | 'export') {
    if (running.current) return
    running.current = true
    setBusy(true)
    setMessage('')
    setError('')
    try {
      if (operation === 'import') {
        const confirmed = await confirmDialog({ title: t('history.import'), message: t('history.confirm'), confirmLabel: t('history.merge') })
        if (!confirmed || !live.current) return
        const report = await host.call<HistoryImportReport | null>('import_history', { confirmed: true })
        if (live.current && report) {
          setMessage(t('history.imported', { created: report.created, merged: report.merged, unchanged: report.unchanged }))
          onImported()
        }
      } else {
        const file = await host.call<FileResult>('export_history')
        if (live.current) setMessage(`${t(file.kind === 'local' ? 'history.saved' : 'history.ready')} ${file.kind === 'local' ? file.path : file.name}`)
      }
    } catch (cause) {
      if (live.current) setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      running.current = false
      if (live.current) setBusy(false)
    }
  }

  return <div className="mw mw-history" aria-busy={busy}>
    <button type="button" className="mw-refresh" disabled={busy} onClick={() => void run('import')}>{t('history.import')}</button>
    <button type="button" className="mw-refresh" disabled={busy} onClick={() => void run('export')}>{t('history.export')}</button>
    {message && <span role="status">{message}</span>}
    {error && <span role="alert">{error}</span>}
  </div>
}
