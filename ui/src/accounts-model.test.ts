import assert from 'node:assert/strict'
import test from 'node:test'
import { accountId, platformLabel, regionLabel, roleLabel } from './accounts-model.ts'
import type { Account, GameRole } from './types.generated'

test('legacy CN identities and global identities display their source without exposing local keys', () => {
  const legacy = { account_key: '123' } as Account
  const global = { account_key: 'global-123', account_id: '123', platform: 'global' } as Account
  const role = { uid: '1800000001', nickname: '角色', region: 'os_asia', region_name: 'Asia' } as GameRole
  assert.equal(platformLabel(legacy.platform), '国服')
  assert.equal(accountId(legacy), '123')
  assert.equal(accountId(global), '123')
  assert.equal(roleLabel(global, role), '国际服 · 角色 · 亚服 · 1800000001')
  assert.equal(regionLabel({ ...role, region: 'future_region', region_name: '新区域' }), '新区域')
})
