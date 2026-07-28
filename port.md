# Dotenv Rust Port: Complete Unified Guide

**A comprehensive guide to porting dotenv from JavaScript to Rust using the Bun methodology.**

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Strategy & Approach](#strategy--approach)
3. [Technical Reference](#technical-reference)
4. [Complete 8-Phase Plan](#complete-8-phase-plan)
5. [Execution Guide](#execution-guide)
6. [Success Criteria](#success-criteria)
7. [Timeline & Effort](#timeline--effort)

---

## Quick Start

**TL;DR:** Create a new Rust project, not a fork. Use adversarial review loops. Test-driven development. 17-24 hours over 2-3 weeks.

### Setup (10 minutes)

```bash
# Create new Rust project (separate from JS dotenv)
cargo new dotenv-rs --lib
cd dotenv-rs

# Project structure
mkdir -p tests/vectors/{basic,quotes,escapes,comments}
mkdir -p docs

# You're ready to start Phase 1
```

### Key Decisions (Already Made)

- ✓ New independent Rust project (not a fork)
- ✓ Reference the original JS code, don't copy it
- ✓ Manual parsing (no regex crate)
- ✓ Adversarial review with split context windows
- ✓ Mechanical port first, idiomatic later
- ✓ 50+ test vectors before coding
- ✓ Full Bun methodology implementation

### What You're Building

A **production-quality Rust library** that:
- Parses `.env` files identically to JS dotenv
- Implements `parse()`, `config()`, `populate()` functions
- Passes 50+ comprehensive test vectors
- Zero unsafe blocks (initially)
- Publishable to crates.io
- Fully documented

### Estimated Effort

- **Solo execution:** 17-24 hours over 2-3 weeks
- **With parallel workflows:** 12-18 hours (better quality)
- **Phases:** 8 phases, 2-6 hours each

---

## Strategy & Approach

### Why: The Problem We're Solving

Porting code is hard. Bun ported 535,496 lines from Zig to Rust in 11 days because they used:
1. **Mechanical porting** (faithful translation)
2. **Adversarial review** (split context = better bugs caught)
3. **Test-driven** (tests first, implementation follows)
4. **Compiler as work queue** (errors = tasks to do)

We're applying the same methodology to dotenv.

### The Reference Strategy

**Do NOT fork the original repository.**

```
motdotla/dotenv (JavaScript)
    │
    └─ (Read Only - Reference)
       Agent reads lib/main.js to understand parsing logic
       Agent learns test cases from their test suite
       Agent never modifies original

dotenv-rs (Your New Project)
    │
    └─ (Write Only - Implementation)
       Agent writes src/parser.rs from scratch
       Agent implements in Rust (different language)
       Agent validates against test vectors

Result:
  ✓ Clean, independent Rust project
  ✓ Semantically equivalent to original
  ✓ Can be published to crates.io
  ✓ Separate maintenance
```

**Why this works:**
- You understand the original
- You implement fresh in Rust
- No code duplication
- Clear separation of concerns
- Ready for independent publication

### Repository Structure

```
dotenv-rs/ (YOUR PROJECT)
├── src/
│   ├── lib.rs              # Public API exports
│   ├── parser.rs           # parse() function
│   ├── config.rs           # config() function  
│   ├── populate.rs         # populate() function
│   └── error.rs            # Error types
├── tests/
│   ├── integration_test.rs # Test harness
│   └── vectors.json        # 50+ test cases
├── Cargo.toml
├── README.md
└── CHANGELOG.md

REFERENCE (don't touch)
https://github.com/motdotla/dotenv/blob/master/lib/main.js
```

### Agent Roles

When using Claude Code or goal-based automation:

**IMPLEMENTER** (Writes code)
- Reads: PORTING_GUIDE, LIFETIMES_GUIDE, test_vectors.json, original JS code
- Writes: src/parser.rs, src/config.rs, etc.
- Goal: Make tests pass

**REVIEWER #1** (Finds logic bugs)
- Reads: ONLY the diff, PORTING_GUIDE, test_vectors.json
- Ignores: Implementer's reasoning, original code (avoid bias)
- Goal: Assume code is wrong, find bugs

**REVIEWER #2** (Compares with original)
- Reads: ONLY the diff, original JS code side-by-side, test_vectors.json
- Goal: Verify semantic equivalence

**IMPLEMENTER** (Feedback loop)
- Reads: Bug reports from both reviewers
- Fixes: All issues found
- Repeats: Until reviewers find 0 new bugs

---

## Technical Reference

### JavaScript to Rust Pattern Mapping

#### String Operations

| JavaScript | Rust | Use Case |
|---|---|---|
| `src.toString()` | `String::from_utf8_lossy(src)` | Convert bytes to string |
| `str.trim()` | `str.trim()` | Remove whitespace |
| `str.replace(regex, repl)` | `str.replace()` or manual | String replacement |
| `str.includes(value)` | `str.contains(value)` | Substring check |
| `str.split('\n')` | `str.lines()` or `str.split()` | Split by delimiter |
| `str.toLowerCase()` | `str.to_lowercase()` | Case conversion |
| `str[0]` | `str.chars().next()` | Character access (UTF-8 safe) |

#### Collections

| JavaScript | Rust |
|---|---|
| `const obj = {}` | `let mut result = HashMap::new()` |
| `obj[key] = value` | `result.insert(key, value)` |
| `Object.keys(obj)` | `result.keys()` |
| `typeof value` | `Value type system` |

#### Error Handling

**JavaScript:**
```javascript
try {
  const parsed = parse(src);
} catch (e) {
  console.error(e.message);
}
```

**Rust:**
```rust
pub enum DotenvError {
    FileNotFound(String),
    IoError(std::io::Error),
    InvalidUtf8,
    ParseError(String),
}

fn parse(src: &[u8]) -> Result<HashMap<String, String>, DotenvError> {
    // ...
}
```

### The Original Regex

The JS dotenv uses this regex:

```regex
(?:^|^)\s*(?:export\s+)?([\w.-]+)(?:\s*=\s*?|:\s+?)
(\s*'(?:\\'|[^'])*'|\s*"(?:\\"|[^"])*"|\s*`(?:\\`|[^`])*`|[^#\r\n]+)?
\s*(?:#.*)?(?:$|$)
```

**What it does:**
1. `(?:^|^)` - Start of line
2. `\s*` - Optional leading whitespace
3. `(?:export\s+)?` - Optional "export " keyword
4. `([\w.-]+)` - Capture group 1: KEY (alphanumeric, dots, hyphens)
5. `(?:\s*=\s*?|:\s+?)` - Equals or colon separator
6. `(...)` - Capture group 2: VALUE (quoted or unquoted)
7. `\s*(?:#.*)?` - Optional trailing comment

**Rust Strategy:** Manual parsing instead of regex

```rust
fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim_start();
    
    // Skip empty lines and comments
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    
    // Skip export keyword
    let line = if line.starts_with("export ") {
        &line[7..]
    } else {
        line
    };
    
    // Find key=value or key:value
    let (key_part, value_part) = if let Some(pos) = line.find('=') {
        (&line[..pos], &line[pos+1..])
    } else if let Some(pos) = line.find(':') {
        (&line[..pos], &line[pos+1..])
    } else {
        return None;
    };
    
    let key = key_part.trim();
    let value = parse_value(value_part.trim());
    
    Some((key.to_string(), value))
}
```

### Quote and Escape Handling

**JavaScript:**
```javascript
const maybeQuote = value[0]
value = value.replace(/^(['"`])([\s\S]*)\1$/mg, '$2')  // Remove quotes
if (maybeQuote === '"') {
  value = value.replace(/\\n/g, '\n')
  value = value.replace(/\\r/g, '\r')
}
```

**Rust:**
```rust
fn parse_value(input: &str) -> String {
    let trimmed = input.trim();
    
    // Detect quote character
    let (quote_char, inner) = match trimmed.chars().next() {
        Some('"') | Some('\'') | Some('`') if trimmed.len() >= 2 => {
            let quote = trimmed.chars().next().unwrap();
            if trimmed.ends_with(quote) {
                (Some(quote), &trimmed[1..trimmed.len()-1])
            } else {
                (None, trimmed)
            }
        }
        _ => (None, trimmed),
    };
    
    // Process escapes only for double-quoted strings
    match quote_char {
        Some('"') => inner
            .replace("\\\\", "\x00")  // Temp marker for backslash
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\\"", "\"")
            .replace("\x00", "\\")    // Restore backslashes
            .to_string(),
        _ => inner.to_string(),
    }
}
```

**Key Differences:**
- Single quotes: No escape processing (literal backslashes)
- Double quotes: Process \n, \r, \\, \"
- Backticks: No escape processing (literal)
- Unquoted: Strip trailing comment with #

### Lifetimes (Simple!)

For dotenv, lifetimes are straightforward:

```rust
pub fn parse(input: &[u8]) -> HashMap<String, String> {
    // Input: borrowed (&[u8] with lifetime 'a)
    // Output: HashMap owns all String keys and values
    // Result: No lifetime parameter needed on output
    
    // Input comes from caller, doesn't need to live long
    // Output is completely independent of input
    // Input can be freed after function returns
}
```

**Why this works:**
- Input is borrowed (temporary reference)
- Parser reads and copies relevant data
- Output owns all data (HashMap<String, String>)
- No references escape the function
- Lifetimes are implicit 'static

---

## Complete 8-Phase Plan

### Phase 1: Test Vectors (2 hours)

**Goal:** Create comprehensive test cases before coding.

**Deliverable:** `tests/vectors.json` with 50+ test cases

**Test Categories:**

```
basic/
  - simple key=value
  - empty values
  - empty file
  - only comments
  - unicode

quotes/
  - single quotes
  - double quotes
  - backticks
  - mixed quotes
  - unbalanced quotes
  - quotes in values

escapes/
  - escape sequences in double quotes
  - NO escape sequences in single quotes
  - newlines (literal and escaped)
  - unicode escapes
  - backslash handling

comments/
  - comments at start of line
  - inline comments after value
  - # inside quotes (NOT a comment)
  - edge cases

export/
  - export keyword
  - export with multiple spaces
  - export mixed with other syntax

multiline/
  - multiline with double quotes
  - multiline with escaped \n
  - edge cases

edge_cases/
  - Windows CRLF (\r\n)
  - Mac CR (\r only)
  - Unix LF (\n only)
  - Mixed line endings
  - Very long lines
  - Binary-like data
  - Spaces in keys (invalid)
  - Dots/hyphens/underscores in keys
```

**Test Vector Format:**

```json
{
  "name": "test_name",
  "category": "quotes",
  "input": "KEY=\"value with spaces\"",
  "expected": {
    "KEY": "value with spaces"
  },
  "description": "Double quoted values preserve spaces"
}
```

**Create Test Harness:** `tests/integration_test.rs`

```rust
#[test]
fn run_all_test_vectors() {
    let test_data = std::fs::read_to_string("tests/vectors.json").unwrap();
    let test_cases: Vec<TestCase> = serde_json::from_str(&test_data).unwrap();

    for test in test_cases {
        let result = dotenv::parse(test.input.as_bytes());
        assert_eq!(result, test.expected,
            "Test '{}' failed.\nExpected: {:?}\nGot: {:?}",
            test.name, test.expected, result);
    }
}
```

### Phase 2: Core Parser Implementation (4-6 hours)

**Goal:** Implement `parse()` function with adversarial review.

**Loop Structure:**

```
Implementer writes parse() → 
  Reviewer #1 finds bugs →
    Reviewer #2 finds discrepancies →
      Implementer fixes →
        [Repeat until 0 new bugs]
```

#### Phase 2a: Implementer

**Task:** Write `src/parser.rs`

```rust
use std::collections::HashMap;

pub fn parse(input: &[u8]) -> HashMap<String, String> {
    let content = String::from_utf8_lossy(input);
    let mut result = HashMap::new();

    // Normalize line endings
    let normalized = content
        .replace("\r\n", "\n")
        .replace("\r", "\n");

    // Parse each line
    for line in normalized.lines() {
        if let Some((key, value)) = parse_line(line) {
            result.insert(key, value);
        }
    }

    result
}

fn parse_line(line: &str) -> Option<(String, String)> {
    // Implementation details in EXECUTION_GUIDE
    todo!()
}

fn parse_value(value: &str) -> String {
    // Implementation details in EXECUTION_GUIDE
    todo!()
}
```

**Deliverable:**
- Code compiles with `cargo check`
- 50+ tests passing
- No panics on any input
- Commit: "phase-2a: implement parse() core logic"

#### Phase 2b: Reviewer #1 (Logic Bugs)

**Context:** ONLY the diff, PORTING_GUIDE, test_vectors.json

**Assume:** Code is wrong

**Check For:**
- Off-by-one errors in string slicing
- Incorrect line ending handling (\r\n vs \n vs \r)
- Quote matching edge cases
- Escape sequence ordering
- Comment detection (# inside quotes shouldn't trigger)
- Export keyword handling
- Unicode edge cases
- Empty values and keys

**Output Format:**

```
BUG #1: Line 42, parse_value()
  Problem: Quote matching doesn't handle escaped quotes
  Example: parse_value(r#""value\"quote""#)
    Returns: "value\"
    Expected: value"quote
  Fix: Check for \\ before removing quote character

BUG #2: ...
```

#### Phase 2c: Reviewer #2 (Semantic Equivalence)

**Context:** ONLY the diff, JS code side-by-side, test_vectors.json

**Check If:**
- Rust handles all cases JS handles
- Behavior is semantically equivalent
- No missing features
- Quotes handled identically
- Escapes processed the same way

**Output Format:**

```
DISCREPANCY #1: Backtick quotes
  JS: Supports backtick quotes, no escape processing
  Rust: Not implemented yet
  Impact: Test 'backtick_handling' will fail
  Fix: Add backtick case to parse_value()

DISCREPANCY #2: ...
```

#### Phase 2d: Implementer (Feedback)

**Apply all feedback:**
1. Read bug/discrepancy
2. Reproduce with test case
3. Fix code
4. Run `cargo test` - all pass
5. Commit: "phase-2b-fix: [description]"
6. Repeat 2b-2d until reviewers find 0 bugs

### Phase 3: File I/O & Config (2-3 hours)

**Goal:** Implement `config()` function for reading .env files.

```rust
pub struct ConfigOptions {
    pub path: Option<String>,
    pub override_existing: bool,
    pub debug: bool,
}

pub fn config(options: Option<ConfigOptions>) 
    -> Result<ConfigResult, DotenvError> {
    // Read file (default: .env in current dir)
    // Call parse()
    // Return result with any errors
}

pub struct ConfigResult {
    pub parsed: HashMap<String, String>,
}

#[derive(Debug)]
pub enum DotenvError {
    FileNotFound(String),
    IoError(std::io::Error),
    InvalidUtf8,
}
```

**Review Focus:**
- File not found handling
- Permission errors
- Encoding issues
- Path resolution
- Multiple file cascading

### Phase 4: Populate Function (1-2 hours)

**Goal:** Implement `populate()` to set environment variables.

```rust
pub fn populate(
    process_env: &mut HashMap<String, String>,
    parsed: &HashMap<String, String>,
    options: Option<PopulateOptions>,
) -> HashMap<String, String> {
    // Set env vars
    // Respect override flag
    // Return what was set
}

pub struct PopulateOptions {
    pub override_existing: bool,
}
```

**Tests:**
- Override behavior correct
- Return value accurate
- Existing vars respected
- Multiple keys handled

### Phase 5: Compiler Errors as Work Queue (2-3 hours)

**Process:**
1. Run `cargo check` → get all errors
2. Group by error type
3. Create work items for each
4. Fix in parallel
5. Validate
6. Repeat until 0 errors

**Example work queue:**
```
[ ] Fix: lifetime issues (3 errors)
[ ] Fix: HashMap type mismatches (5 errors)
[ ] Fix: Missing impl Clone (2 errors)
[ ] Fix: Result unwrap patterns (4 errors)
```

### Phase 6: Integration Testing (2-3 hours)

**Run full test suite:**
```bash
cargo test --all
# Should: test result: ok. 50 passed; 0 failed
```

**Comparison testing:**
- Load each test vector
- Run parse()
- Compare output
- Assert byte-for-byte match

**Platform testing:**
- Windows (PowerShell)
- macOS
- Linux
- Line endings on all platforms

### Phase 7: Performance & Cleanup (1-2 hours)

**Benchmark:**
```bash
cargo bench
```

Compare with JS dotenv on:
- Small files (< 1KB)
- Medium files (1-10KB)
- Large files (10MB+)

**Code Review:**
- Replace loops with iterators where appropriate
- Use pattern matching effectively
- Reduce allocations
- Document public API

**Polish:**
```bash
cargo clippy --all-targets  # Zero warnings
cargo doc --open            # Check docs
```

### Phase 8: Documentation & Publishing (1-2 hours)

**Create:**
- `README.md` - Usage guide and examples
- Doc comments on all public functions
- `CHANGELOG.md` - Version history
- Examples in `examples/` directory

**Publish to crates.io:**
```bash
cargo login
cargo publish
```

---

## Execution Guide

### Setting Up Each Phase

#### Phase 1: Test Vectors

**Goal Command:**
```
"Create comprehensive test vectors for dotenv parse()"
```

**Concrete Steps:**
1. Create `tests/vectors.json`
2. Add 50+ test cases (use template above)
3. Create `tests/integration_test.rs` with test harness
4. Run: `cargo test` - all tests fail (expected)
5. Commit: `git commit -m "phase-1: add test vectors"`

#### Phase 2: Parser with Adversarial Review

**Goal Command:**
```
"Implement parse() function with adversarial review until all tests pass"
```

**Steps:**

```
LOOP:
  1. IMPLEMENTER writes src/parser.rs
     $ cargo check  (must compile)
     $ cargo test   (should fail at first)
     Commit: "phase-2a: implement parse()"

  2. REVIEWER_1 reads diff
     Output: Bug list

  3. REVIEWER_2 reads diff
     Output: Discrepancy list

  4. Feedback exists?
     YES → Go to step 5
     NO  → Phase 2 complete

  5. IMPLEMENTER applies feedback
     $ cargo test   (run tests)
     Commit: "phase-2b-fix: [description]"
     Go to step 2 (new review pass)
```

#### Phase 3-4: Same Pattern

Same review loop as Phase 2, just different functions.

#### Phase 5: Compiler Errors

**Steps:**
1. Run `cargo check 2>&1 | tee errors.txt`
2. Group errors by type
3. Create fix for each group
4. Test
5. Repeat

#### Phase 6: Testing

**Steps:**
```bash
# Run all tests
cargo test --all

# Run specific test category
cargo test quote_handling

# With output
cargo test -- --nocapture
```

#### Phase 7: Cleanup

```bash
# Check for warnings
cargo clippy --all-targets

# Format code
cargo fmt

# Check documentation
cargo doc --open
```

#### Phase 8: Publishing

```bash
# Update Cargo.toml version
# Update README
# Tag release
git tag v1.0.0

# Publish
cargo publish

# Verify
cargo search dotenv-rs
```

### Git Commit Convention

```
phase-1: add test vectors
phase-2a: implement parse() core logic
phase-2b-fix: handle quote edge cases
phase-2c-fix: correct line ending logic
phase-3: implement config() file I/O
phase-4: implement populate() function
phase-5: fix compiler errors
phase-6: add comprehensive tests
phase-7: optimize and cleanup
phase-8: document and prepare for publish
```

### Loop Template (for Claude Code)

```javascript
const phases = [
  { name: "Test Vectors", duration: "2 hours" },
  { name: "Parser", duration: "6 hours", review_loops: true },
  { name: "Config", duration: "2 hours", review_loops: true },
  { name: "Populate", duration: "1 hour", review_loops: true },
  { name: "Compiler Errors", duration: "2 hours" },
  { name: "Testing", duration: "2 hours" },
  { name: "Cleanup", duration: "1 hour" },
  { name: "Publishing", duration: "1 hour" }
];

for (const phase of phases) {
  console.log(`Starting: ${phase.name}`);
  
  if (phase.review_loops) {
    while (true) {
      const impl = await runImplementer(phase);
      const bugs1 = await reviewerLogicBugs(impl);
      const bugs2 = await reviewerCompareWithJS(impl);
      const allBugs = [...bugs1, ...bugs2];
      
      if (allBugs.length === 0) break;
      
      await implementerApplyFeedback(allBugs);
    }
  } else {
    await runPhase(phase);
  }
  
  console.log(`✓ ${phase.name} complete\n`);
}
```

---

## Success Criteria

Port is complete when:

```
✓ Phase 1: 50+ test vectors created
✓ Phase 2: parse() passes all vectors
✓ Phase 3: config() reads .env files
✓ Phase 4: populate() sets env vars
✓ Phase 5: Zero compiler errors
✓ Phase 6: All integration tests pass
✓ Phase 7: cargo clippy shows 0 warnings
✓ Phase 8: Published to crates.io

Quality Gates:
✓ Byte-for-byte match with JS dotenv output
✓ Zero unsafe blocks (or justified comments)
✓ 100% public API documentation
✓ Tested on Windows, macOS, Linux
✓ Performance comparable to JS version
✓ All edge cases handled

Final:
✓ GitHub repo created
✓ crates.io published
✓ README complete with examples
✓ CHANGELOG documenting port
✓ ✓ All reviewers signed off (0 bugs found)
```

---

## Timeline & Effort

### Breakdown by Phase

| Phase | Task | Hours | Depends On |
|---|---|---|---|
| 1 | Create test vectors | 2 | Nothing |
| 2a | Implement parse() | 2 | Phase 1 |
| 2b | Review logic bugs | 1 | Phase 2a |
| 2c | Review vs original | 1 | Phase 2a |
| 2d | Apply feedback | 1 | Phase 2b-c |
| 2 | Repeat loop | +2-3 | Until 0 bugs |
| 3 | Config function | 2-3 | Phase 2 complete |
| 4 | Populate function | 1-2 | Phase 3 complete |
| 5 | Compiler errors | 2-3 | Phase 4 complete |
| 6 | Integration tests | 2-3 | Phase 5 complete |
| 7 | Cleanup & polish | 1-2 | Phase 6 complete |
| 8 | Docs & publish | 1-2 | Phase 7 complete |

**Total: 17-24 hours over 2-3 weeks**

### Daily/Weekly Pace Options

**Option A: Intensive (1 week)**
- 3-4 hours/day × 5 days = 15-20 hours

**Option B: Moderate (2 weeks)**
- 2 hours/day × 5 days/week = 20 hours total

**Option C: Relaxed (3 weeks)**
- 1-2 hours/day × 5 days/week = 25-30 hours total (with buffer)

### Realistic Timeline

Most likely: 2-3 weeks at 1-2 hours per day

Key factors:
- **Phase 1:** Quick (just creating JSON test cases)
- **Phase 2:** Longest (parse logic + 2-3 review passes)
- **Phases 3-4:** Quick (simpler functions)
- **Phase 5:** Usually fast (compiler errors are clear)
- **Phase 6-8:** Medium (testing, cleanup, documentation)

---

## Common Pitfalls to Avoid

### ❌ Mistake #1: Copy-Pasting Code

**Wrong:** Copying the JavaScript code directly

**Right:** Understanding the logic, implementing fresh in Rust

### ❌ Mistake #2: Skipping Review

**Wrong:** One person writes, no external review

**Right:** Separate implementer from reviewers with split context windows

### ❌ Mistake #3: Over-Engineering

**Wrong:** Refactoring to "idiomatic Rust" immediately

**Right:** Mechanical port first, refactor in v1.1

### ❌ Mistake #4: Incomplete Test Vectors

**Wrong:** Testing only happy paths

**Right:** 50+ test cases covering edge cases

### ❌ Mistake #5: Forking the Original

**Wrong:** Adding Rust code to motdotla/dotenv

**Right:** Creating new dotenv-rs project

---

## Publishing Checklist

Before publishing to crates.io:

```
Code Quality:
  [ ] cargo test passes
  [ ] cargo clippy shows 0 warnings
  [ ] cargo fmt applied
  [ ] No unsafe blocks (or documented)

Documentation:
  [ ] README.md complete
  [ ] All public functions documented
  [ ] CHANGELOG.md up to date
  [ ] Examples in examples/ directory
  [ ] API docs render correctly: cargo doc --open

Testing:
  [ ] 50+ test vectors passing
  [ ] Platform tests (Windows, macOS, Linux)
  [ ] Benchmarks comparable to JS
  [ ] No regressions

Metadata:
  [ ] Cargo.toml version updated
  [ ] License file added (MIT)
  [ ] Repository URL correct
  [ ] Keywords and categories set

Publishing:
  [ ] cargo login (authenticate with crates.io)
  [ ] cargo publish --dry-run (test publish)
  [ ] cargo publish (actual publish)
  [ ] Verify: cargo search dotenv-rs
  [ ] GitHub release notes added
```

---

## Key Insights from Bun Methodology

### 1. Mechanical Port First

Don't refactor to "idiomatic Rust" initially. Stay close to original structure so:
- Review is easier (reviewers can compare side-by-side)
- Bugs are caught earlier (less hidden complexity)
- Refactoring happens after v1.0

### 2. Separated Reviewers = Better Bugs Caught

One implementer + two independent reviewers:
- Reviewer #1 assumes code is wrong (finds logic bugs)
- Reviewer #2 compares with original (finds semantic bugs)
- Together: catch bugs implementer misses

### 3. Compiler Errors as Work Queue

Don't be demoralized by 16,000 compiler errors (Bun had that).
Each error is a task. Group and parallelize:
- Make progress visible
- Don't get overwhelmed
- Keep momentum

### 4. Test-Driven Development

Define success BEFORE coding:
- 50+ test cases first
- Implementation targets passing tests
- Tests = specification

### 5. Commit Often

Each small win = 1 commit:
- phase-2a: implement core
- phase-2b-fix: handle quotes
- phase-2c-fix: line endings
- phase-2d-fix: exports

Small commits make history clear and reversible.

---

## What Success Looks Like

### At End of Phase 1
```
✓ tests/vectors.json: 50+ test cases
✓ tests/integration_test.rs: Test harness
✓ cargo test: All 50+ tests fail (expected, haven't implemented yet)
```

### At End of Phase 2
```
✓ src/parser.rs: Fully implemented
✓ cargo test: All 50+ tests pass
✓ No compiler warnings
✓ Reviewers found 0 new bugs last pass
✓ Byte-for-byte match with JS dotenv
```

### At End of Phase 4
```
✓ parse() ✓ config() ✓ populate()
✓ src/error.rs: Error types
✓ All tests passing
✓ No unsafe blocks
```

### At End of Phase 8
```
✓ Published to crates.io
✓ Full documentation
✓ 100% test coverage
✓ Benchmarks comparable
✓ All platforms tested
✓ Production-ready Rust library
```

---

## Next Steps

### Right Now
1. Read this document (you're doing it!)
2. Understand the strategy (reference-based porting)
3. Decide: solo vs parallel workflows

### In 10 Minutes
```bash
cargo new dotenv-rs --lib
cd dotenv-rs
mkdir tests
```

### Then Start Phase 1
- Create `tests/vectors.json`
- Add 50+ test cases
- Create test harness
- Commit!

### Then Execute Phases 2-8
- Follow the review loop template
- Keep momentum
- Celebrate each completed phase

---

## Support & Resources

### The Documents
- **This file:** Complete unified guide
- **Original code:** https://github.com/motdotla/dotenv/blob/master/lib/main.js
- **JS test suite:** https://github.com/motdotla/dotenv/tree/master/tests

### When You Get Stuck
1. Re-read the relevant phase section
2. Check the pattern mapping (above)
3. Review the test vectors (what's expected?)
4. Look at the original JS implementation
5. Ask: "What would the original do here?"

### Remember
- Mechanical port (faithful, not idiomatic)
- Review process (separate roles)
- Test-driven (tests first)
- Small commits (clear history)
- You've got this! 🚀

---

**Start Phase 1. You're ready!**
