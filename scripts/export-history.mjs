// Read-only bridge for an older plugin that has no export button. Login data is never read.
import { parseArgs } from 'node:util'
import { lstat, readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'

const { values } = parseArgs({ options: { 'data-dir': { type: 'string' }, output: { type: 'string' } } })
if (!values['data-dir'] || !values.output) throw new Error('Use --data-dir <old plugin data directory> --output <new backup.json>.')
const root = path.resolve(values['data-dir'], 'series-v2')
const series = []
async function directory(file) {
  const metadata = await lstat(file)
  if (metadata.isSymbolicLink() || !metadata.isDirectory()) throw new Error('History directories must be real directories.')
  return readdir(file, { withFileTypes: true })
}
for (const account of await directory(root)) {
  for (const region of await directory(path.join(root, account.name))) {
    for (const entry of await directory(path.join(root, account.name, region.name))) {
      if (!entry.name.endsWith('.json')) continue
      if (!entry.isFile()) throw new Error('History entries must be ordinary files.')
      const record = JSON.parse(await readFile(path.join(root, account.name, region.name, entry.name), 'utf8'))
      if (record.schema_version !== 2 || record.scope?.account_key !== account.name
        || record.scope?.region !== region.name || `${record.scope?.uid}.json` !== entry.name) throw new Error('History metadata does not match its path.')
      const { schema_version, scope, updated_at, fetch_count, overview, stages, details } = record
      series.push({ schema_version, scope, updated_at, fetch_count, overview, stages, details })
    }
  }
}
series.sort((a, b) => JSON.stringify(a.scope).localeCompare(JSON.stringify(b.scope)))
await writeFile(path.resolve(values.output), JSON.stringify({ format: 'wonderland-my-wonderland-history', schema_version: 1, series }), { flag: 'wx' })
console.log(`Exported ${series.length} role histories. Account login records were excluded.`)
