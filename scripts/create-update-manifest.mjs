#!/usr/bin/env node

import { createHash, createPrivateKey, sign } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { basename } from 'node:path'

const [, , manifestPath, packagePath, repository, tag, outputPath] = process.argv
if (!manifestPath || !packagePath || !repository || !tag || !outputPath) {
  throw new Error('Usage: node scripts/create-update-manifest.mjs <manifest.json> <package.wplug> <owner/repo> <tag> <output.json>')
}

const encodedSeed = process.env.PLUGIN_UPDATE_SIGNING_KEY ?? ''
const seed = Buffer.from(encodedSeed, 'base64')
if (seed.length !== 32 || seed.toString('base64') !== encodedSeed) {
  throw new Error('PLUGIN_UPDATE_SIGNING_KEY must be the canonical base64 encoding of a 32-byte Ed25519 seed')
}

const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
  throw new Error('Repository must be an owner/repository pair')
}
if (!manifest.id || !manifest.version || !manifest.hostCompatibility || !manifest.platform || manifest.manifestVersion !== 3) {
  throw new Error('Plugin manifest is missing required update metadata')
}
if (tag !== manifest.version && tag !== `v${manifest.version}`) {
  throw new Error('Release tag must match the plugin manifest version')
}

const packageName = `${manifest.id}-${manifest.version}-windows-${manifest.platform.architecture}.wplug`
if (basename(packagePath) !== packageName) {
  throw new Error(`Release package must be named ${packageName}`)
}

const packageBytes = readFileSync(packagePath)
const payload = {
  schemaVersion: 1,
  id: manifest.id,
  version: manifest.version,
  releaseNotesUrl: `https://github.com/${repository}/releases/tag/${tag}`,
  downloadUrl: `https://github.com/${repository}/releases/download/${tag}/${packageName}`,
  sha256: createHash('sha256').update(packageBytes).digest('hex'),
  sizeBytes: packageBytes.length,
  hostCompatibility: manifest.hostCompatibility,
  uiBridgeCompatibility: manifest.ui?.bridgeCompatibility ?? null,
  platform: manifest.platform,
  dependencies: manifest.dependencies ?? [],
}

const privateKeyDer = Buffer.concat([
  Buffer.from('302e020100300506032b657004220420', 'hex'),
  seed,
])
const privateKey = createPrivateKey({ key: privateKeyDer, format: 'der', type: 'pkcs8' })
const payloadText = JSON.stringify(payload)
const signature = sign(null, Buffer.from(payloadText, 'utf8'), privateKey).toString('hex')
const envelope = { schemaVersion: 1, payload: payloadText, signature }
writeFileSync(outputPath, `${JSON.stringify(envelope, null, 2)}\n`, { mode: 0o644 })
console.log(`Created signed update manifest for ${manifest.id} v${manifest.version}`)
