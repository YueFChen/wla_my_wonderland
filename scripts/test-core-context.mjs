// Run the actual backend with a fake Core and temporary plugin data only.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { createInterface } from 'node:readline'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const binary = path.resolve(process.argv[2] ?? path.join(root, 'target/debug/wonderland-my-wonderland.exe'))
const data = await mkdtemp(path.join(tmpdir(), 'my-wonderland-context-'))
await mkdir(path.join(data, 'archive-v1'))
const oldPath = path.join(data, 'archive-v1', 'existing.json')
const oldBytes = Buffer.from('existing plugin data is retained')
await writeFile(oldPath, oldBytes)
const child = spawn(binary, [], { windowsHide: true, env: { ...process.env, WONDERLAND_PLUGIN_DATA_DIR: data }, stdio: ['pipe', 'pipe', 'pipe'] })
const envelope = { protocol: 'wonderland-plugin', version: '1.0.0' }
const send = value => child.stdin.write(`${JSON.stringify({ ...envelope, ...value })}\n`)
const lines = createInterface({ input: child.stdout })
const pending = new Map(), authenticated = []
const snapshot = { status: 'logged_in', current_account_key: 'demo', last_login_failure: null, accounts: [{ account_key: 'demo', game_roles: [{ uid: '100001', region: 'cn_gf01', region_name: 'Fixture', nickname: 'Fixture', level: 60 }] }] }
const loggedOut = { status: 'logged_out', current_account_key: null, last_login_failure: null, accounts: [] }
let hello, failure, stderr = '', serial = 0, releaseFirst
const firstReply = new Promise(resolve => { releaseFirst = resolve })
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
child.stderr.on('data', bytes => { stderr += bytes })
child.on('error', error => { failure ??= error })

async function core(frame) {
  if (frame.method === 'core.account.snapshot') {
    return frame.serviceContext === 'collect-device' ? snapshot : loggedOut
  }
  assert.equal(frame.method, 'core.account.authed_get')
  assert.equal(frame.serviceContext, 'collect-device', 'Another device must not overwrite the active collection context')
  authenticated.push(frame)
  if (authenticated.length === 1) await firstReply
  const fixture = frame.params.path.endsWith('get_user_info') ? 'overview' : 'empty'
  const bytes = await readFile(path.join(root, 'tests/fixtures', `${fixture}.json`))
  return { contentBase64: bytes.toString('base64') }
}

lines.on('line', line => {
  try {
    const frame = JSON.parse(line)
    if (frame.type === 'hello') { hello = frame; return }
    if (frame.type === 'event') return
    if (frame.type === 'request') {
      void core(frame).then(result => send({ type: 'result', id: frame.id, result }), error => {
        failure ??= error
        send({ type: 'error', id: frame.id, error: { code: 'TEST_CORE_FAILURE', message: error.message } })
      })
      return
    }
    const call = pending.get(frame.id)
    assert(call, `Unknown response ${frame.id}`)
    pending.delete(frame.id)
    if (frame.type === 'error') call.reject(new Error(JSON.stringify(frame.error)))
    else call.resolve(frame.result)
  } catch (error) { failure ??= error }
})
async function until(predicate) {
  const deadline = Date.now() + 15000
  while (!predicate()) {
    if (failure) throw failure
    assert.equal(child.exitCode, null, stderr)
    assert(Date.now() < deadline, 'Test fixture timed out')
    await pause(10)
  }
}
function rpc(method, params, serviceContext) {
  const id = `fixture-${++serial}`
  return new Promise((resolve, reject) => { pending.set(id, { resolve, reject }); send({ type: 'request', id, method, params, serviceContext }) })
}

try {
  send({ type: 'hello', role: 'host', pluginId: 'my_wonderland' })
  await until(() => hello)
  const manifest = JSON.parse(await readFile(path.join(root, 'package/manifest.json'), 'utf8'))
  assert.equal(hello.pluginVersion, manifest.version)
  const collect = rpc('collect', { scope: { account_key: 'demo', uid: '100001', region: 'cn_gf01' } }, 'collect-device')
  await until(() => authenticated.length === 1)
  const other = await rpc('account_snapshot', {}, 'another-device')
  assert.equal(other.status, 'logged_out')
  releaseFirst()
  const result = await Promise.race([collect, pause(15000).then(() => { throw new Error('Collection timed out') })])
  assert.equal(result.fetch_count, 1)
  assert.equal(authenticated.length, 2, 'The second call validates against the collection snapshot and retains its context')
  assert.deepEqual(await readFile(oldPath), oldBytes)
  if (failure) throw failure
  console.log(`PASS: my_wonderland collection context, snapshot and existing data; fixture ${data}`)
} finally {
  releaseFirst()
  child.stdin.end()
  await Promise.race([new Promise(resolve => child.once('exit', resolve)), pause(1000)])
  if (child.exitCode === null) child.kill()
  lines.close()
}
