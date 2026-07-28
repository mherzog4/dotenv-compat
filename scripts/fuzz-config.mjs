// Differential fuzzer for config(): file cascade, the override flag, keys that
// are already set, and missing files.
//
//   cd scripts && npm install && node fuzz-config.mjs [iterations] [seed]
//
// The JavaScript side uses `processEnv` to get a clean target object. The Rust
// side has no such option and must use the real process environment, so every
// generated key carries a reserved prefix -- otherwise a collision with a real
// variable would make the two sides disagree about what is already set.

import { execFileSync } from 'node:child_process'
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRequire } from 'node:module'

import { Reader, Writer, makeRng, oracleBin, sameMap } from './harness.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const require = createRequire(import.meta.url)
const dotenv = require('dotenv')

const ITERATIONS = Number(process.argv[2] ?? 3000)
const SEED = Number(process.argv[3] ?? 1)
const BATCH = 250

const PREFIX = 'ZZFUZZ_'
const KEYS = [`${PREFIX}A`, `${PREFIX}B`, `${PREFIX}C`, `${PREFIX}D`]
const VALUES = ['one', 'two', '', '"quoted value"', "'single'", 'with # comment', 'a\\nb', '  padded  ']

const { rnd, pick, chance, int } = makeRng(SEED)

const workdir = join(here, '..', 'target', 'fuzz-config')
rmSync(workdir, { recursive: true, force: true })
mkdirSync(workdir, { recursive: true })

let fileCounter = 0

/** One case: some files (a few deliberately absent), some preset variables, a flag. */
function genCase () {
  const paths = []
  for (let i = 0; i < 1 + int(3); i++) {
    const path = join(workdir, `f${fileCounter++}.env`)
    if (chance(0.2)) {
      paths.push(path) // deliberately not written -- exercises the missing-file branch
      continue
    }
    const lines = []
    for (let k = 0; k < 1 + int(3); k++) {
      lines.push(chance(0.15) ? '# just a comment' : `${pick(KEYS)}=${pick(VALUES)}`)
    }
    writeFileSync(path, lines.join('\n') + (chance(0.5) ? '\n' : ''))
    paths.push(path)
  }

  const presets = []
  for (const key of KEYS) {
    if (chance(0.35)) presets.push([key, `preset_${Math.floor(rnd() * 1000)}`])
  }

  return { paths, presets, overwrite: chance(0.5) }
}

function encode (cases) {
  const w = new Writer()
  for (const c of cases) {
    w.u32(c.paths.length)
    for (const p of c.paths) w.field(p)
    w.u32(c.presets.length)
    for (const [k, v] of c.presets) { w.field(k); w.field(v) }
    w.byte(c.overwrite ? 1 : 0)
  }
  return w.done()
}

/** What the reference implementation does with the same case. */
function reference (c) {
  const processEnv = Object.fromEntries(c.presets)
  const result = dotenv.configDotenv({
    path: c.paths,
    override: c.overwrite,
    quiet: true,
    processEnv
  })

  const touched = new Set([...Object.keys(result.parsed), ...c.presets.map(([k]) => k)])
  const resulting = {}
  for (const key of touched) {
    if (processEnv[key] !== undefined) resulting[key] = processEnv[key]
  }

  return { hadError: !!result.error, parsed: result.parsed, resulting }
}

const bin = oracleBin()
const failures = []
let checked = 0

for (let done = 0; done < ITERATIONS; done += BATCH) {
  const cases = []
  for (let i = 0; i < Math.min(BATCH, ITERATIONS - done); i++) cases.push(genCase())

  const reader = new Reader(
    execFileSync(bin, ['config'], { input: encode(cases), maxBuffer: 1 << 28 })
  )

  for (const c of cases) {
    checked++
    const actual = { hadError: reader.byte() !== 0, parsed: reader.map(), resulting: reader.map() }
    const expected = reference(c)

    if (
      actual.hadError !== expected.hadError ||
      !sameMap(actual.parsed, expected.parsed) ||
      !sameMap(actual.resulting, expected.resulting)
    ) {
      failures.push({ case: c, expected, actual })
    }
  }
}

rmSync(workdir, { recursive: true, force: true })

if (failures.length) {
  console.error(`FAIL: ${failures.length}/${checked} mismatches (seed ${SEED})`)
  for (const f of failures.slice(0, 5)) console.error(JSON.stringify(f, null, 2))
  process.exit(1)
}

console.log(`ok: ${checked} config cases agree with dotenv@${require('dotenv/package.json').version} (seed ${SEED})`)
