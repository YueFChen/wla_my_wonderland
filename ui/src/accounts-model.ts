import type { Account, GameRole, Platform } from './types.generated'

export function platformLabel(platform: Platform = 'cn'): string {
  return platform === 'global' ? '国际服' : '国服'
}

export function accountId(account: Account): string {
  return account.account_id ?? account.account_key
}

export function regionLabel(role: GameRole): string {
  const labels: Record<string, string> = {
    os_usa: '美服', os_euro: '欧服', os_asia: '亚服', os_cht: '台港澳服',
  }
  return labels[role.region] || role.region_name || role.region
}

export function roleLabel(account: Account, role: GameRole): string {
  return `${platformLabel(account.platform)} · ${role.nickname} · ${regionLabel(role)} · ${role.uid}`
}
