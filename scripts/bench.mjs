// Times the reference implementation on the same inputs as examples/bench.rs.
//
//   cd scripts && node bench.mjs

import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const dotenv = require('dotenv')

/** Must stay byte-identical to `synthetic()` in examples/bench.rs. */
function synthetic (lines) {
  let out = ''
  for (let i = 0; i < lines; i++) {
    switch (i % 5) {
      case 0: out += `PLAIN_${i}=value${i}\n`; break
      case 1: out += `QUOTED_${i}="value ${i} with spaces"\n`; break
      case 2: out += `SINGLE_${i}='literal \\n stays ${i}'\n`; break
      case 3: out += `# comment about key ${i}\nCOMMENTED_${i}=v${i} # trailing\n`; break
      default: out += `export MULTI_${i}="line one\\nline two ${i}"\n`
    }
  }
  return out
}

for (const [label, lines] of [['small', 20], ['medium', 400], ['large', 40000]]) {
  const input = synthetic(lines)
  const bytes = Buffer.byteLength(input)
  const reps = Math.min(20000, Math.max(3, Math.floor(20_000_000 / bytes)))

  for (let i = 0; i < Math.min(reps, 50); i++) dotenv.parse(input) // warm up the JIT

  const start = process.hrtime.bigint()
  let keys = 0
  for (let i = 0; i < reps; i++) keys += Object.keys(dotenv.parse(input)).length
  const elapsed = Number(process.hrtime.bigint() - start) / 1e9

  const perOp = elapsed / reps
  console.log(
    `node  ${label.padEnd(7)} ${String(bytes).padStart(9)} B  ` +
    `${(perOp * 1e6).toFixed(1).padStart(10)} us/op  ` +
    `${(bytes / perOp / 1e6).toFixed(1).padStart(7)} MB/s  (${keys / reps} keys)`
  )
}
