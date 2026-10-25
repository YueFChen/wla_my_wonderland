import { useEffect, useRef, useState } from 'react'
import { Drawer, DialogButton, confirmDialog } from '@wonderland/ui'
import type { Account, AccountSnapshot, Platform } from './types.generated'
import { t } from './i18n'
import { accountId, platformLabel, regionLabel } from './accounts-model'
import './accounts.css'

export interface AccountApi {
  login(platform: Platform): Promise<AccountSnapshot>
  cancel(): Promise<unknown>
  sync(key: string): Promise<Account>
  switch(key: string): Promise<unknown>
  remove(key: string): Promise<unknown>
  refresh(): Promise<void>
}

export function AccountManagement({ open, onClose, snapshot, api }: {
  open: boolean; onClose: () => void; snapshot: AccountSnapshot | null; api: AccountApi
}) {
  const [platform, setPlatform] = useState<Platform>('cn')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])
  const run = async (action: () => Promise<unknown>) => {
    setBusy(true)
    setError('')
    try { await action() }
    catch (cause) { if (mounted.current) setError(cause instanceof Error ? cause.message : String(cause)) }
    finally { await api.refresh(); if (mounted.current) setBusy(false) }
  }
  const logging = snapshot?.status === 'logging_in'
  const forget = async (account: Account) => {
    if (await confirmDialog({ title: t('account.forgetTitle'), message: t('account.forgetMessage'), confirmLabel: t('account.forget'), danger: true })) {
      await run(() => api.remove(account.account_key))
    }
  }
  const failures = { window_closed: t('account.windowClosed'), timeout: t('account.timeout'), cancelled: t('account.cancelled'), failed: t('account.failed') }
  return <Drawer open={open} dismissible={!busy} title={t('account.title')} onClose={onClose} closeLabel={t('account.close')} size="lg">
    <div className="mw-accounts">
      <div className="mw-accounts-actions">
        {logging ? <>
          <span role="status">{t('account.logging')}</span>
          <DialogButton onClick={() => void api.cancel().then(() => api.refresh()).catch(cause => setError(String(cause)))}>{t('account.cancelLogin')}</DialogButton>
        </> : <>
          <select className="app-input" aria-label={t('account.platform')} value={platform} disabled={busy} onChange={event => setPlatform(event.target.value as Platform)}>
            <option value="cn">{platformLabel('cn')}</option>
            <option value="global">{platformLabel('global')}</option>
          </select>
          <DialogButton variant="primary" disabled={busy} onClick={() => void run(() => api.login(platform))}>{t('account.login')}</DialogButton>
        </>}
      </div>
      {error && <p className="mw-account-error" role="alert">{error}</p>}
      {!logging && snapshot?.last_login_failure && !error && <p className="mw-accounts-status" role="status">{failures[snapshot.last_login_failure]}</p>}
      {(snapshot?.accounts ?? []).map(account => {
        const avatar = account.game_roles[0]?.creator?.avatar_url ?? account.avatar_url
        return <article className="mw-account" key={account.account_key}>
          <div className="mw-account-head">
            <div className="mw-account-avatar">{avatar ? <img src={avatar} alt="" /> : <span>{(account.game_roles[0]?.nickname ?? accountId(account)).slice(0, 1)}</span>}</div>
            <div className="mw-account-copy">
              <strong>{account.game_roles[0]?.nickname ?? accountId(account)}</strong>
              <span>{account.platform === 'global' ? 'HoYoverse ID' : t('account.passport')} {accountId(account)}</span>
            </div>
            <span className="mw-account-platform">{platformLabel(account.platform)}</span>
            {snapshot?.current_account_key === account.account_key && <span className="mw-account-default">{t('account.default')}</span>}
          </div>
          {account.game_roles.length > 0 && <ul className="mw-account-roles">{account.game_roles.map((role, index) => {
            const level = role.creator?.level ?? (index === 0 ? account.creator_level : null)
            return <li key={`${role.region}/${role.uid}`}>
              <span>{role.nickname}{level != null && <small>{t('account.creator')} Lv.{level}</small>}</span>
              <span>{regionLabel(role)} · {role.uid} · Lv.{role.level}</span>
            </li>
          })}</ul>}
          <div className="mw-account-buttons">
            <DialogButton disabled={busy || logging} onClick={() => void run(() => api.sync(account.account_key))}>{t('account.sync')}</DialogButton>
            {snapshot?.current_account_key !== account.account_key && <DialogButton disabled={busy || logging} onClick={() => void run(() => api.switch(account.account_key))}>{t('account.setDefault')}</DialogButton>}
            <DialogButton variant="danger" disabled={busy || logging} onClick={() => void forget(account)}>{t('account.forget')}</DialogButton>
          </div>
        </article>
      })}
      {snapshot?.accounts.length === 0 && !logging && <p className="mw-accounts-status">{t('account.empty')}</p>}
    </div>
  </Drawer>
}
