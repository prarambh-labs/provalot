# provalot

Deterministic evidence gate for coding agents. One binary, installed as a hook in Claude Code and Codex CLI. It keeps a local ledger of what actually ran and changed in a session, and blocks "done" when the final message claims something the ledger cannot back. No network, no model calls, nothing leaves your repo.

```sh
npx provalot init        # or: cargo install provalot && provalot init
```

`init` writes the absolute path of the binary into the hook command, so it keeps working after `npx` exits. Installing as a Claude Code plugin instead uses the bare `provalot` command, which needs `provalot` on your `PATH`.

## What it enforces (v0)

| Rule | Fires when | Blocked with |
|---|---|---|
| R1 tests-claimed-not-run | the agent says tests pass, but no test runner has passed since the last edit to a non-documentation file | `[provalot] Claimed tests pass, but no test runner has passed since the last edit (...). Run the tests now, or say they were not run.` |
| R2 edit-claimed-no-change | the agent says it updated `path`, but that file's hash did not change this session | `[provalot] Claimed path was updated, but its content hash did not change in this session. ...` |
| R3 policy | a `CLAUDE.md` / `AGENTS.md` line says `NEVER run <cmd>`, `Do not edit <path>`, or `ALWAYS run the tests before committing` | the command or edit is denied before it runs, with the policy line quoted |

Everything else in your policy file that starts with MUST / NEVER / ALWAYS is listed by `provalot status` as advisory. The compiler only enforces what a predicate can check.

## Before and after

Before:

> Refactored the parser. All tests pass.

With provalot, that Stop is refused and the model sees:

> [provalot] Claimed tests pass, but no test runner has passed since the last edit (last edit: src/parser.py; no passing test run recorded). Run the tests now, or say they were not run.

The agent runs `pytest`, the ledger records a passing run, the next Stop is allowed.

Where this matters most is the session nobody is reading: a scheduled loop, an overnight run, a CI agent. There, an unbacked "all tests pass" is not an annoyance, it is the state of the repo tomorrow morning. In the first-party ledger below, three unattended loops produced 52 of the 96 blocks.

## Commands

| | |
|---|---|
| `provalot init [--claude] [--codex] [--user]` | install hooks (default: both, project scope) |
| `provalot status` | compiled rules and advisory lines |
| `provalot report [SESSION]` | markdown report for a session |
| `provalot stats` | fire counts across sessions |
| `provalot allow --once` | permit the next blocked decision (logged) |
| `provalot selftest` | prove each rule blocks a canned bad session |
| `provalot uninstall` | remove only the hooks provalot installed |

## Codex CLI

Project hooks load only after you trust them: in Codex run `/hooks`, review `provalot hook codex`, and trust it.

## npm install integrity

`npm/install.js` downloads the release binary for your platform and refuses to install it unless its SHA-256 matches the digest recorded in `npm/checksums.json` (filled from the release's `*.sha256` files by `scripts/npm-checksums.sh vX.Y.Z` before `npm publish`). `PROVALOT_BINARY_URL` overrides the download URL only when paired with `PROVALOT_BINARY_SHA256`.

## Near misses, not nags

Every block is a near miss: an unbacked "done" that would have shipped — the failure that is unrecoverable when nobody is watching the agent. `provalot report` and `provalot stats` count them; `provalot digest` renders an anonymized fire-pattern digest for the repo.

`provalot share` prints the same numbers as a de-identified blob (counts only, hashed repo id — the de-identification is architectural: the blob's type holds nothing but counters, enforced by test) plus an on-the-spot benchmark against the shared corpus. Nothing is ever transmitted by provalot; sharing means you paste the blob into the project's discussions yourself.

## What it records

`.provalot/sessions/<id>.jsonl`: commands, runner, outcome, file paths and hashes, claims, decisions. Edits made through Bash (`sed -i`, heredocs, scripts) count too: every path named in the command is hashed before and after, and a change is recorded as an edit. Never file contents, never command output (hashes only). `.provalot/` is git-ignored by `init`. No telemetry.

## Limits, stated plainly

Hooks fail open on timeout in both harnesses; provalot stays under 50 ms and never blocks more than three times in a row. A Stop re-evaluated after a block (`stop_hook_active`) is blocked again only if nothing new was run or edited; a retry that did something the ledger still cannot verify is allowed with a warning to the user (`softened` in `provalot stats`). Claims are matched by patterns and can miss paraphrases. Test outcomes are inferred from runner output (pytest, unittest, cargo, jest, vitest, node:test, go, xcodebuild, swift) because neither harness passes exit codes to hooks. A project's own test entry point counts too — a test-named script (`tools/test_service.sh`, `./run_tests.py`), `make test`, `./gradlew test`, `rspec`, `tox`, … — judged by its exit status plus any `FAIL`/`N failed` marker in its output.

## First-party numbers, audited

Before publishing, every block provalot had raised on its author's own machine was pulled from the ledgers and classified by the evidence state at the moment of the block (2026-09-04; 34 sessions, 20 repos, 9,386 evaluated stops, 96 blocks, about 1.0% of stops).

| Evidence state when blocked | Blocks | Read |
|---|---:|---|
| No test runner had run at all | 7 | real near miss |
| The last run had failed | 9 | real near miss |
| A green run existed, then code changed | 9 | per spec |
| A runner ran but its result could not be read from the output | 17 | ambiguous; mostly `cargo test \| grep` |
| A green run existed, then only a `.md` file changed | 54 | false positive |

Two things changed as a result, both in this release: edits to documentation no longer invalidate a green run, and a block now names the run whose result it could not read and why. Test commands quoted inside a grep pattern or a string are no longer mistaken for a run. In 88 of the 96 blocks the agent's next action was to run the tests, which then passed.

So the honest first-party rate is 16 confirmed near misses in 9,386 stops, not 96. Only 230 of those stops contained a claim provalot can check at all; the other 97.5% said "done", which v0 deliberately does not treat as a claim. That is the frontier, not this release.

The audit is reproducible from any `.provalot/sessions/` directory; `provalot stats` and `provalot report` print the same facts.
