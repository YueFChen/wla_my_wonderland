import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
test('legacy history export excludes login files and never overwrites a backup', async context => {
  await mkdir(path.join(root, 'target'), { recursive: true })
  const directory = await mkdtemp(path.join(root, 'target/history-export-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const history = path.join(directory, 'series-v2/demo/cn_gf01')
  await mkdir(history, { recursive: true })
  await mkdir(path.join(directory, 'account'), { recursive: true })
  await writeFile(path.join(directory, 'account/accounts.json'), 'synthetic-private-login')
  const record = { schema_version: 2, scope: { account_key: 'demo', uid: '100001', region: 'cn_gf01' }, updated_at: 1, fetch_count: 1, overview: {}, stages: [], details: {} }
  await writeFile(path.join(history, '100001.json'), JSON.stringify(record))
  const output = path.join(directory, 'backup.json')
  const run = () => spawnSync(process.execPath, ['scripts/export-history.mjs', '--data-dir', directory, '--output', output], { cwd: root, encoding: 'utf8' })
  const result = run()
  assert.equal(result.status, 0, result.stderr)
  const bytes = await readFile(output, 'utf8')
  assert.deepEqual(JSON.parse(bytes).series, [record])
  assert(!bytes.includes('synthetic-private-login'))
  assert.notEqual(run().status, 0)
  assert.equal(await readFile(output, 'utf8'), bytes)
})
