// Real backend, synthetic browser/HTTP and isolated DPAPI data. No real credentials or network.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, writeFile, cp } from 'node:fs/promises'
import path from 'node:path'
import { createInterface } from 'node:readline'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const binary = path.resolve(process.argv[2] ?? path.join(root, 'target/debug/wonderland-wla-my-wonderland.exe'))
await mkdir(path.join(root, 'target'), { recursive: true })
const base = await mkdtemp(path.join(root, 'target', 'platform-review-'))
const data = path.join(base, 'plugin'), legacy = path.join(base, 'legacy')
await mkdir(path.join(legacy, 'account/credentials'), { recursive: true })
await mkdir(path.join(data, 'archive-v1'), { recursive: true })
const original = Buffer.from('{"accounts":[],"current_account":""}')
const sealed = Buffer.from('synthetic old orphan encrypted bytes')
await writeFile(path.join(legacy, 'account/accounts.json'), original)
await writeFile(path.join(legacy, 'account/credentials/orphan.bin'), sealed)
await writeFile(path.join(data, 'archive-v1/keep'), 'retained')

const scope = { account_key: 'demo', uid: '100001', region: 'cn_gf01' }
const regions = ['os_asia', 'os_usa', 'os_euro', 'os_cht']
const globalRoles = regions.map((region, index) => ({
  uid: String(1800000001 + index), region, region_name: region,
  role_nickname: 'Fixture-' + region, role_level: 5 + index,
}))
// Deliberately different hours test provider metadata, not actual production schedules.
const hours = { cn_gf01: 6, os_asia: 6, os_usa: 18, os_euro: 12, os_cht: 0 }
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
const fixture = async name => JSON.parse(await readFile(path.join(root, 'tests/fixtures', name + '.json'), 'utf8'))
function inScope(value, uid, region) {
  if (Array.isArray(value)) return value.map(item => inScope(item, uid, region))
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, value]) => [
    key, key === 'uid' ? uid : key === 'region' ? region : inScope(value, uid, region),
  ]))
  return value
}

class Harness {
  constructor() {
    this.pending = new Map(); this.streams = new Map(); this.sessions = new Set()
    this.authenticated = []; this.serial = 0; this.issueCookies = false; this.stderr = ''
    this.accountId = 'demo'; this.platform = 'cn'; this.nonempty = false
    this.child = spawn(binary, [], {
      windowsHide: true,
      env: { ...process.env, WONDERLAND_PLUGIN_DATA_DIR: data, WONDERLAND_HOST_DATA_DIR: legacy },
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    this.child.stderr.on('data', bytes => { this.stderr += bytes })
    this.child.on('error', error => { this.failure = error })
    this.lines = createInterface({ input: this.child.stdout })
    this.lines.on('line', line => {
      try {
        const frame = JSON.parse(line)
        if (frame.type === 'hello') { this.hello = frame; return }
        if (frame.type === 'event') return
        if (frame.type === 'request') {
          void this.core(frame).then(
            result => this.send({ type: 'result', id: frame.id, result }),
            error => {
              this.failure = error
              this.send({ type: 'error', id: frame.id, error: { code: 'FIXTURE_FAILURE', message: error.message } })
            },
          )
          return
        }
        const call = this.pending.get(frame.id)
        assert(call, 'Unknown backend response')
        this.pending.delete(frame.id); clearTimeout(call.timer)
        frame.type === 'error'
          ? call.reject(Object.assign(Error(frame.error.message), { code: frame.error.code }))
          : call.resolve(frame.result)
      } catch (error) { this.failure = error }
    })
    this.send({ type: 'hello', role: 'host', pluginId: 'wla_my_wonderland' })
  }
  send(frame) { this.child.stdin.write(JSON.stringify({ protocol: 'wonderland-plugin', version: '1.0.0', ...frame }) + '\n') }
  async until(predicate) {
    const deadline = Date.now() + 15000
    while (!predicate()) {
      if (this.failure) throw this.failure
      assert.equal(this.child.exitCode, null, this.stderr)
      assert(Date.now() < deadline, 'Fixture timeout')
      await pause(10)
    }
  }
  request(method, params = {}, serviceContext = 'device') {
    const id = 'fixture-' + (++this.serial)
    const promise = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('RPC timeout: ' + method)), 20000)
      this.pending.set(id, { resolve, reject, timer })
      this.send({ type: 'request', id, method, params, serviceContext })
    })
    void promise.catch(() => {}) // Concurrent probes may inspect state before awaiting this request.
    return { id, promise }
  }
  rpc(method, params = {}, context = 'device') { return this.request(method, params, context).promise }
  async core(frame) {
    assert(!frame.method.startsWith('core.account.'), 'Accounts must remain plugin-owned')
    const p = frame.params ?? {}
    switch (frame.method) {
      case 'core.files.pick':
        assert.equal(frame.serviceContext, 'history-device')
        assert.deepEqual(p.extensions, ['json'])
        return this.cancelPick ? { cancelled: true } : { cancelled: false, handle: 'history-fixture' }
      case 'core.files.read': {
        assert.equal(frame.serviceContext, 'history-device')
        assert.equal(p.handle, 'history-fixture')
        const chunk = this.importBytes.subarray(p.offset, p.offset + p.chunkBytes)
        return { offset: p.offset, contentBase64: chunk.toString('base64'), eof: p.offset + chunk.length >= this.importBytes.length }
      }
      case 'core.files.export_open':
        assert.equal(frame.serviceContext, 'history-device')
        assert.equal(p.name, 'my-wonderland-history.json')
        this.exportBytes = Buffer.alloc(0)
        return { streamId: 'history-export' }
      case 'core.streams.write':
        assert.equal(frame.serviceContext, 'history-device')
        assert.equal(p.streamId, 'history-export')
        this.exportBytes = Buffer.concat([this.exportBytes, Buffer.from(p.contentBase64, 'base64')])
        return { ok: true }
      case 'core.streams.finish':
        assert.equal(frame.serviceContext, 'history-device')
        assert.equal(p.streamId, 'history-export')
        return { kind: 'remote', state: 'ready', name: 'my-wonderland-history.json', size: this.exportBytes.length, downloadId: 'history-download' }
      case 'core.browser.session.open':
        assert.equal(frame.serviceContext, this.loginContext)
        this.platform = p.profile === 'hoyoverse-login' ? 'global' : 'cn'
        assert.equal(p.profile, this.platform === 'global' ? 'hoyoverse-login' : 'mihoyo-login')
        assert.equal(new URL(p.url).hostname, this.platform === 'global' ? 'act.hoyoverse.com' : 'act.mihoyo.com')
        assert.equal(p.reset, true); this.sessions.add('browser')
        return { sessionId: 'browser' }
      case 'core.browser.session.cookies': {
        assert(this.sessions.has(p.sessionId))
        const domain = this.platform === 'global' ? 'hoyoverse.com' : '.mihoyo.com'
        return this.issueCookies ? [
          { name: 'ltuid_v2', value: this.accountId, domain, path: '/' },
          { name: 'ltoken_v2', value: 'synthetic-cookie-token-' + this.platform, domain, path: '/' },
          // The other provider's ID and token must not leak into the selected account.
          { name: 'account_id_v2', value: 'unrelated', domain: this.platform === 'global' ? '.mihoyo.com' : '.hoyoverse.com', path: '/' },
          { name: 'cookie_token_v2', value: 'other-provider', domain: this.platform === 'global' ? '.mihoyo.com' : '.hoyoverse.com', path: '/' },
        ] : []
      }
      case 'core.browser.session.navigate':
      case 'core.browser.session.visibility': return { ok: true }
      case 'core.browser.session.close': this.sessions.delete(p.sessionId); return { ok: true }
      case 'core.network.request': {
        const url = new URL(p.url)
        for (const [name, value] of p.query ?? []) url.searchParams.set(name, value)
        const global = url.hostname === 'sg-api-hoyocreators.hoyoverse.com'
        assert(['api-takumi.mihoyo.com', 'api-micreator.mihoyo.com', 'sg-api-hoyocreators.hoyoverse.com'].includes(url.hostname))
        assert.equal(url.searchParams.get('game_biz'), global ? 'hk4e_global' : 'hk4e_cn')
        const header = name => Buffer.from(p.headers.find(h => h.name.toLowerCase() === name)?.value ?? '').toString('utf8')
        const cookie = header('cookie')
        assert(cookie.includes('synthetic-cookie-token-' + (global ? 'global' : 'cn')), 'Cookie provider mismatch at ' + url.hostname + url.pathname + '; keys=' + cookie.split('; ').map(item => item.split('=')[0]).join(','))
        assert(!cookie.includes('other-provider'))
        assert(!cookie.includes('unrelated'))
        if (url.hostname !== 'api-takumi.mihoyo.com') {
          assert.equal(header('origin'), global ? 'https://act.hoyoverse.com' : 'https://act.mihoyo.com')
        }
        this.authenticated.push({ path: url.pathname, context: frame.serviceContext, url: url.href })
        let body
        if (url.pathname.endsWith('/creator/homepage')) {
          body = this.rejectLogin ? { retcode: -100, data: null } : { retcode: 0, data: {
            aid: this.wrongAid ? 'another-account' : this.accountId, data_center_refresh_hours: hours,
          } }
        } else if (url.pathname.endsWith('getUserGameRolesByCookie')) {
          body = { retcode: 0, data: { list: [{ game_biz: 'hk4e_cn', game_uid: '100001', region: 'cn_gf01', region_name: 'Fixture', nickname: 'Fixture', level: 60 }] } }
        } else if (url.pathname.endsWith('/creator/role_account/list')) {
          body = { retcode: 0, data: { list: globalRoles } }
        } else if (url.pathname.includes('/creator')) {
          const roleIndex = Math.max(0, regions.indexOf(url.searchParams.get('region')))
          body = { retcode: 0, data: { kolugc_level: roleIndex + 2, cur_level: roleIndex + 2, cur_exp: 0, total_exp: 100, ra_game_info: { avatar_url: '' } } }
        } else {
          assert.equal(frame.serviceContext, 'collect-device', 'A concurrent request changed collection ownership')
          if (this.hold && url.pathname.endsWith('get_user_info')) await this.hold
          const uid = url.searchParams.get('uid'), region = url.searchParams.get('region')
          const name = url.pathname.endsWith('get_user_info') ? 'overview'
            : url.pathname.endsWith('get_stage_detail') ? 'detail'
              : this.nonempty ? (url.searchParams.get('page_index') === '2' ? 'page2' : 'page1') : 'empty'
          body = inScope(await fixture(name), uid, region)
          if (name === 'detail') body.data.stage_info.stage_id = url.searchParams.get('stage_id')
        }
        const id = 'body-' + this.serial + '-' + this.authenticated.length
        const bytes = Buffer.from(JSON.stringify(body))
        this.streams.set(id, { bytes, context: frame.serviceContext })
        return { status: 200, headers: [], finalUrl: url.href, body: { streamId: id, size: bytes.length } }
      }
      case 'core.streams.read': {
        const stream = this.streams.get(p.streamId); assert(stream)
        assert.equal(frame.serviceContext, stream.context); this.streams.delete(p.streamId)
        return { contentBase64: stream.bytes.toString('base64'), eof: true }
      }
      case 'core.streams.close': this.streams.delete(p.streamId); return { ok: true }
      default: throw Error('Unexpected host service ' + frame.method)
    }
  }
  async stop() {
    if (this.stopped) return
    this.stopped = true
    for (const item of this.pending.values()) clearTimeout(item.timer)
    this.child.stdin.end()
    await Promise.race([new Promise(resolve => this.child.once('exit', resolve)), pause(700)])
    if (this.child.exitCode === null) this.child.kill()
    this.lines.close()
  }
}

let first = new Harness(), second, third
try {
  await first.until(() => first.hello)
  assert.equal((await first.rpc('account_snapshot')).accounts.length, 0)
  assert.deepEqual(await readFile(path.join(data, 'account/accounts.json')), original)
  assert.deepEqual(await readFile(path.join(data, 'account/credentials/orphan.bin')), sealed)
  first.issueCookies = true; first.loginContext = 'login-device'
  const logged = await first.rpc('account_login', {}, 'login-device')
  assert.equal(logged.current_account_key, 'demo')
  assert.equal(logged.accounts[0].platform, 'cn')
  assert.equal(logged.accounts[0].game_roles[0].uid, '100001')
  assert.equal(first.sessions.size, 0)
  assert(!(await readFile(path.join(data, 'account/credentials/demo.bin'))).includes(Buffer.from('synthetic-cookie-token')))
  await cp(path.join(data, 'account'), path.join(base, 'accepted-login/account'), { recursive: true })
  await first.stop()

  second = new Harness()
  await second.until(() => second.hello)
  assert.equal((await second.rpc('account_snapshot')).current_account_key, 'demo')
  let release
  second.hold = new Promise(resolve => { release = resolve })
  const collecting = second.rpc('collect', { scope }, 'collect-device')
  await second.until(() => second.authenticated.length > 0)
  assert.equal((await second.rpc('account_snapshot', {}, 'another-device')).accounts[0].account_key, 'demo')
  release()
  assert.equal((await collecting).fetch_count, 1)
  assert.equal(second.authenticated.length, 2)

  second.loginContext = 'cancel-device'
  const pending = second.request('account_login', {}, 'cancel-device')
  await second.until(() => second.sessions.size === 1)
  second.send({ type: 'cancel', id: 'unrelated' })
  assert.equal((await second.rpc('account_snapshot')).status, 'logging_in')
  second.send({ type: 'cancel', id: pending.id })
  assert.equal((await pending.promise).last_login_failure, 'cancelled')
  assert.equal(second.sessions.size, 0)

  second.issueCookies = true; second.loginContext = 'global-login-device'
  const international = await second.rpc('account_login', { platform: 'global' }, 'global-login-device')
  assert.equal(international.accounts.length, 2)
  assert.equal(international.current_account_key, 'global-demo')
  const global = international.accounts.find(account => account.platform === 'global')
  assert.equal(global.account_id, 'demo')
  assert.equal(global.game_roles.length, 4)
  assert.deepEqual(global.game_roles.map(role => role.creator.level), [2, 3, 4, 5])
  assert.deepEqual(global.game_roles.map(role => role.creator.exp), [0, 0, 0, 0])
  assert.deepEqual(global.game_roles.map(role => role.data_refresh_hour), [6, 18, 12, 0])
  const globalSeries = []
  for (const role of global.game_roles) {
    second.nonempty = role.region === 'os_asia'
    const selected = { account_key: global.account_key, uid: role.uid, region: role.region }
    const result = await second.rpc('collect', { scope: selected }, 'collect-device')
    assert.equal(result.scope.region, role.region)
    if (second.nonempty) assert(Object.keys(result.details).length > 0)
    globalSeries.push(selected)
  }
  assert.equal((await second.rpc('series', { scope })).fetch_count, 1, 'Global data replaced CN data')
  await assert.rejects(second.rpc('collect', { scope: { ...globalSeries[0], account_key: 'demo' } }, 'collect-device'), { code: 'PLUGIN_MY_WONDERLAND_INVALID_INPUT' })
  await assert.rejects(second.rpc('account_login', { platform: 'unknown' }, 'global-login-device'), { code: 'INVALID_INPUT' })

  second.accountId = 'rejected'; second.rejectLogin = true
  const beforeValidation = second.authenticated.length
  const rejected = second.request('account_login', { platform: 'global' }, 'global-login-device')
  await second.until(() => second.authenticated.length > beforeValidation)
  assert.equal((await second.rpc('account_snapshot')).accounts.length, 2, 'Unverified cookies became an account')
  second.send({ type: 'cancel', id: rejected.id })
  await rejected.promise
  second.rejectLogin = false; second.wrongAid = true
  await assert.rejects(second.rpc('account_login', { platform: 'global' }, 'global-login-device'), { code: 'PLUGIN_MY_WONDERLAND_INVALID_RESPONSE' })
  assert.equal((await second.rpc('account_snapshot')).accounts.length, 2)
  assert.equal(second.sessions.size, 0)

  await second.rpc('account_remove', { account_key: 'global-demo' })
  assert.equal((await second.rpc('account_snapshot')).accounts.length, 1)
  for (const selected of globalSeries) assert.equal((await second.rpc('series', { scope: selected })).fetch_count, 1)
  await second.rpc('account_remove', { account_key: 'demo' })
  await second.stop()

  third = new Harness()
  await third.until(() => third.hello)
  assert.equal((await third.rpc('account_snapshot')).accounts.length, 0)
  assert.equal((await third.rpc('series', { scope })).fetch_count, 1)
  for (const selected of globalSeries) assert.equal((await third.rpc('series', { scope: selected })).fetch_count, 1)
  // Exercise the actual file RPC boundary, including remote context and cancelled selection.
  const receipt = await third.rpc('export_history', {}, 'history-device')
  assert.equal(receipt.kind, 'remote')
  assert.equal(receipt.size, third.exportBytes.length)
  const backup = JSON.parse(third.exportBytes)
  assert.equal(backup.series.length, 5)
  assert(!third.exportBytes.includes(Buffer.from('synthetic-cookie-token')))
  await assert.rejects(third.rpc('import_history', {}, 'history-device'), { code: 'CONFIRMATION_REQUIRED' })
  third.cancelPick = true
  assert.equal(await third.rpc('import_history', { confirmed: true }, 'history-device'), null)
  third.cancelPick = false
  third.importBytes = third.exportBytes
  assert.deepEqual(await third.rpc('import_history', { confirmed: true }, 'history-device'), { created: 0, merged: 0, unchanged: 5 })
  third.importBytes = Buffer.from(JSON.stringify({ ...backup.series[0], scope: { ...backup.series[0].scope, account_key: 'imported' } }))
  assert.deepEqual(await third.rpc('import_history', { confirmed: true }, 'history-device'), { created: 1, merged: 0, unchanged: 0 })
  assert.deepEqual(await readFile(path.join(legacy, 'account/accounts.json')), original)
  assert.deepEqual(await readFile(path.join(legacy, 'account/credentials/orphan.bin')), sealed)
  assert.equal(await readFile(path.join(data, 'archive-v1/keep'), 'utf8'), 'retained')
  if (first.failure || second.failure || third.failure) throw first.failure ?? second.failure ?? third.failure
  console.log('PASS: CN/global login, four regions, collection, encrypted restart, cancellation, history import/export and device context. Fixture: ' + base)
} finally {
  await first.stop()
  if (second) await second.stop()
  if (third) await third.stop()
}
