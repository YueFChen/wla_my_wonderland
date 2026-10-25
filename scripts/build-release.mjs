import { finishPackage } from '../../../packages/plugin-tools/pack.mjs'
import './check-core-compatibility.mjs'
import { createHash } from 'node:crypto'
import { cp, mkdir, readFile, rm, stat } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
if (process.platform !== 'win32' || process.arch !== 'x64') {
  throw new Error('The wla_my_wonderland release package targets Windows x86_64 MSVC.')
}

const manifest = JSON.parse(await readFile(path.join(root, 'package', 'manifest.json'), 'utf8'))
const packageJson = JSON.parse(await readFile(path.join(root, 'package.json'), 'utf8'))
const uiPackageJson = JSON.parse(await readFile(path.join(root, 'ui', 'package.json'), 'utf8'))
const cargoToml = await readFile(path.join(root, 'Cargo.toml'), 'utf8')
const workspaceVersion = cargoToml.match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1]

if (!manifest || manifest.id !== 'wla_my_wonderland' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/.test(manifest.version)) {
  throw new Error('package/manifest.json must define the wla_my_wonderland ID and a valid SemVer version.')
}
if ([packageJson.version, uiPackageJson.version, workspaceVersion].some((version) => version !== manifest.version)) {
  throw new Error('manifest, root package, UI package, and Rust workspace versions must match.')
}
if (process.env.GITHUB_REF_TYPE === 'tag' && process.env.GITHUB_REF_NAME !== `v${manifest.version}`) {
  throw new Error(`Release tag must be v${manifest.version}.`)
}
if (manifest.platform?.os !== 'windows' || manifest.platform?.architecture !== 'x86_64' || manifest.platform?.abi !== 'msvc') {
  throw new Error('The release manifest must target windows/x86_64/msvc.')
}

function run(command, args, cwd = root) {
  const isWindowsPnpm = process.platform === 'win32' && command === 'pnpm'
  const executable = isWindowsPnpm ? (process.env.ComSpec ?? 'cmd.exe') : command
  const commandArgs = isWindowsPnpm
    ? ['/d', '/s', '/c', ['pnpm.cmd', ...args].map((arg) => {
        if (!/^[\w./:-]+$/.test(arg)) throw new Error(`Unsupported Windows command argument: ${arg}`)
        return arg
      }).join(' ')]
    : args
  const result = spawnSync(executable, commandArgs, { cwd, stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}

const targetRoot = path.resolve(root, 'target')
function targetPath(...parts) {
  const resolved = path.resolve(targetRoot, ...parts)
  const relative = path.relative(targetRoot, resolved)
  if (!relative || relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error('Refusing to write a release artifact outside this repository target directory.')
  }
  return resolved
}

const stagingRoot = targetPath('release-staging', 'wla-my-wonderland-plugin')
const releaseRoot = targetPath('release')
const executable = path.join(targetRoot, 'release', 'wonderland-wla-my-wonderland.exe')
const archiveName = `${manifest.id}-${manifest.version}-windows-${manifest.platform.architecture}.wplug`
const archivePath = targetPath(archiveName)

run('pnpm', ['--dir', 'ui', 'run', 'build'])
run('cargo', ['build', '--locked', '--release', '--package', 'wonderland-wla-my-wonderland', '--bin', 'wonderland-wla-my-wonderland'])

await rm(stagingRoot, { recursive: true, force: true })
await mkdir(path.join(stagingRoot, 'backend'), { recursive: true })
await mkdir(releaseRoot, { recursive: true })
await cp(path.join(root, 'package', 'manifest.json'), path.join(stagingRoot, 'manifest.json'))
await cp(path.join(root, 'package', 'contract.json'), path.join(stagingRoot, 'contract.json'))
await cp(path.join(root, 'ui', 'dist'), path.join(stagingRoot, 'ui'), { recursive: true })
await cp(executable, path.join(stagingRoot, 'backend', 'wonderland-wla-my-wonderland.exe'))

await finishPackage({root,stage:stagingRoot,archivePath})

const archiveSize = (await stat(archivePath)).size
if (archiveSize < 1 || archiveSize > 100 * 1024 * 1024) {
  throw new Error('The .wplug archive must be between 1 byte and 100 MiB.')
}
const archiveSha256 = createHash('sha256').update(await readFile(archivePath)).digest('hex')
console.log(JSON.stringify({
  package: path.relative(root, archivePath).split(path.sep).join('/'),
  version: manifest.version,
  sizeBytes: archiveSize,
  sha256: archiveSha256,
}, null, 2))
