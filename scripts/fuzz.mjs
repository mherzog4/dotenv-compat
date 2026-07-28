// Differential fuzzer: generates .env-shaped inputs, parses each with the reference
// JavaScript dotenv and with the Rust port, and reports any disagreement.
//
//   cd scripts && npm install && node fuzz.mjs [iterations] [seed]
//
// Mismatches are printed and also appended to fuzz-failures.json so they can be
// promoted into tests/vectors.json.

import { execFileSync } from 'node:child_process'
import { writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRequire } from 'node:module'

import { oracleBin } from './harness.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const require = createRequire(import.meta.url)
const dotenv = require('dotenv')

const ITERATIONS = Number(process.argv[2] ?? 20000)
const SEED = Number(process.argv[3] ?? 1)
const BATCH = 2000

// xorshift32, so a failing run is reproducible from its seed.
let state = SEED >>> 0 || 1
function rnd () {
  state ^= state << 13; state >>>= 0
  state ^= state >>> 17
  state ^= state << 5; state >>>= 0
  return state / 0x100000000
}
const pick = arr => arr[Math.floor(rnd() * arr.length)]
const chance = p => rnd() < p

const KEYS = ['A', 'KEY', 'a.b', 'a-b', '_x', '123', 'export', 'K', 'lower', 'A B', '', 'k$', 'kéy']
const SEPS = ['=', ' = ', '  =', '=\t', ':', ': ', ':\t', ':  ', '\n=', '=\n', ' :', '==']
const VALUES = [
  '', 'v', 'value with spaces', '  padded  ', '#c', 'a#b', 'a #b',
  "'sq'", '"dq"', '`bt`', "'un", '"un', '`un',
  "'a\\'b'", '"a\\"b"', '`a\\`b`', "'a\\'b", '"a\\"b', 'a\\nb', '"a\\nb"', "'a\\nb'",
  '"a\nb"', "'a\nb'", '`a\nb`', '"a\nb', '""', "''", '``', '"', "'", '`',
  '"a" "b"', '"a" trail', 'a"b', 'C:\\new\\dir', '"C:\\new\\dir"', '{"j":1}',
  '\\', '\\\\', '"\\\\"', 'x\u00a0y', '\ufeff', '\t', '   ', '"#"', "'#'"
]
const NOISE = ['', '   ', '\t', '# comment', '  # c', '---', 'JUSTAKEY', 'export', '=v', '#', '"', "'",
  'A=1\u2028B=2', 'A="#"\u2028C=3', '__proto__=x', 'constructor=y']
const SOUP = [...`A=1'"\`\\#: \t\nxyz.-_`, '\u00a0', '\ufeff', '\u0000', 'é',
  // U+2028/U+2029 are JS line terminators for ^ $ and . but are NOT excluded by
  // [^#\\r\\n]; a reviewer found a real divergence here that this pool had missed.
  '\u2028', '\u2029', '\u0085', '\u{1d11e}']

function genLine () {
  if (chance(0.2)) return pick(NOISE)
  const exp = chance(0.2) ? pick(['export ', 'export  ', 'export\t', 'export\n', 'export']) : ''
  return exp + pick(KEYS) + pick(SEPS) + pick(VALUES)
}

function genInput () {
  if (chance(0.35)) {
    // Unstructured soup finds backtracking bugs that grammar-shaped input misses.
    const n = Math.floor(rnd() * 40)
    let s = ''
    for (let i = 0; i < n; i++) s += pick(SOUP)
    return s
  }
  const n = 1 + Math.floor(rnd() * 4)
  const lines = []
  for (let i = 0; i < n; i++) lines.push(genLine())
  return lines.join(pick(['\n', '\n', '\n', '\r\n', '\r', '\u2028', '\u2029']))
}

function encodeBatch (inputs) {
  const parts = []
  for (const s of inputs) {
    const bytes = Buffer.from(s, 'utf8')
    const len = Buffer.alloc(4)
    len.writeUInt32LE(bytes.length)
    parts.push(len, bytes)
  }
  return Buffer.concat(parts)
}

function decodeBatch (buf, count) {
  const results = []
  let off = 0
  for (let i = 0; i < count; i++) {
    const pairs = buf.readUInt32LE(off); off += 4
    const map = {}
    for (let p = 0; p < pairs; p++) {
      const kl = buf.readUInt32LE(off); off += 4
      const key = buf.subarray(off, off + kl).toString('utf8'); off += kl
      const vl = buf.readUInt32LE(off); off += 4
      const val = buf.subarray(off, off + vl).toString('utf8'); off += vl
      map[key] = val
    }
    results.push(map)
  }
  return results
}

const bin = oracleBin()
const failures = []
let checked = 0

for (let done = 0; done < ITERATIONS; done += BATCH) {
  const inputs = []
  for (let i = 0; i < Math.min(BATCH, ITERATIONS - done); i++) inputs.push(genInput())

  const got = decodeBatch(
    execFileSync(bin, { input: encodeBatch(inputs), maxBuffer: 1 << 28 }),
    inputs.length
  )

  inputs.forEach((input, i) => {
    checked++
    const expected = dotenv.parse(input)
    const actual = got[i]
    const keys = new Set([...Object.keys(expected), ...Object.keys(actual)])
    for (const k of keys) {
      if (expected[k] !== actual[k]) {
        failures.push({ input, expected, actual })
        return
      }
    }
  })
}

if (failures.length) {
  const out = join(here, 'fuzz-failures.json')
  writeFileSync(out, JSON.stringify(failures.slice(0, 200), null, 2))
  console.error(`FAIL: ${failures.length}/${checked} mismatches (seed ${SEED}); first 200 written to ${out}`)
  for (const f of failures.slice(0, 10)) {
    console.error(`  input:    ${JSON.stringify(f.input)}`)
    console.error(`  expected: ${JSON.stringify(f.expected)}`)
    console.error(`  actual:   ${JSON.stringify(f.actual)}`)
  }
  process.exit(1)
}

console.log(`ok: ${checked} inputs agree with dotenv@${require('dotenv/package.json').version} (seed ${SEED})`)
