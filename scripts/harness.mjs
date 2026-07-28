// Shared plumbing for the differential fuzzers: seeded RNG, the path to the Rust
// oracle binary, and the length-prefixed frame codec both sides speak.

import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))

/** xorshift32, so a failing run is reproducible from its seed. */
export function makeRng (seed) {
  let state = seed >>> 0 || 1
  const rnd = () => {
    state ^= state << 13; state >>>= 0
    state ^= state >>> 17
    state ^= state << 5; state >>>= 0
    return state / 0x100000000
  }
  return {
    rnd,
    pick: arr => arr[Math.floor(rnd() * arr.length)],
    chance: p => rnd() < p,
    int: n => Math.floor(rnd() * n)
  }
}

export function oracleBin () {
  const base = join(here, '..', 'target', 'release', 'examples', 'oracle')
  const path = process.platform === 'win32' ? `${base}.exe` : base
  if (!existsSync(path)) {
    console.error(`missing ${path}\nbuild it first: cargo build --release --example oracle`)
    process.exit(2)
  }
  return path
}

export class Writer {
  constructor () { this.parts = [] }

  u32 (n) {
    const b = Buffer.alloc(4)
    b.writeUInt32LE(n)
    this.parts.push(b)
    return this
  }

  byte (n) { this.parts.push(Buffer.from([n])); return this }

  field (value) {
    const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value, 'utf8')
    this.u32(bytes.length)
    this.parts.push(bytes)
    return this
  }

  done () { return Buffer.concat(this.parts) }
}

export class Reader {
  constructor (buf) { this.buf = buf; this.at = 0 }

  u32 () { const v = this.buf.readUInt32LE(this.at); this.at += 4; return v }

  byte () { return this.buf[this.at++] }

  field () {
    const len = this.u32()
    const s = this.buf.subarray(this.at, this.at + len).toString('utf8')
    this.at += len
    return s
  }

  map () {
    const n = this.u32()
    const out = {}
    for (let i = 0; i < n; i++) {
      const key = this.field()
      out[key] = this.field()
    }
    return out
  }
}

/** Compare two plain string maps, ignoring key order. */
export function sameMap (a, b) {
  const keys = new Set([...Object.keys(a), ...Object.keys(b)])
  for (const k of keys) if (a[k] !== b[k]) return false
  return true
}
