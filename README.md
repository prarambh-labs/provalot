# provalot

Deterministic evidence gate for coding agents. One binary, installed as a hook in Claude Code and Codex CLI. It keeps a local ledger of what actually ran and changed in a session, and blocks "done" when the final message claims something the ledger cannot back. No network, no model calls, nothing leaves your repo.

```sh
npx provalot init        # or: cargo install provalot && provalot init
```

`init` writes the absolute path of the binary into the hook command, so it keeps working after `npx` exits. Installing as a Claude Code plugin instead uses the bare `provalot` command, which needs `provalot` on your `PATH`.

## What it enforces (v0)

| Rule | Fires when | Blocked with |
|---|---|---|
| R1 tests-claimed-not-run | the agent says tests pass, but no test runner has passed since the last edit | `[provalot] Claimed tests pass, but no test runner has passed since the last edit (...). Run the tests now, or say they were not run.` |
| R2 edit-claimed-no-change | the agent says it updated `path`, but that file's hash did not change this session | `[provalot] Claimed path was updated, but its content hash did not change in this session. ...` |
| R3 policy | a `CLAUDE.md` / `AGENTS.md` line says `NEVER run <cmd>`, `Do not edit <path>`, or `ALWAYS run the tests before committing` | the command or edit is denied before it runs, with the policy line quoted |

Everything else in your policy file that starts with MUST / NEVER / ALWAYS is listed by `provalot status` as advisory. The compiler only enforces what a predicate can check.

## Before and after

Before:

> Refactored the parser. All tests pass.

With provalot, that Stop is refused and the model sees:

> [provalot] Claimed tests pass, but no test runner has passed since the last edit (last edit: src/parser.py; no passing test run recorded). Run the tests now, or say they were not run.

The agent runs `pytest`, the ledger records a passing run, the next Stop is allowed.

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

## What it records

`.provalot/sessions/<id>.jsonl`: commands, runner, outcome, file paths and hashes, claims, decisions. Edits made through Bash (`sed -i`, heredocs, scripts) count too: every path named in the command is hashed before and after, and a change is recorded as an edit. Never file contents, never command output (hashes only). `.provalot/` is git-ignored by `init`. No telemetry.

## Limits, stated plainly

Hooks fail open on timeout in both harnesses; provalot stays under 50 ms and never blocks more than three times in a row. A Stop re-evaluated after a block (`stop_hook_active`) is blocked again only if nothing new was run or edited; a retry that did something the ledger still cannot verify is allowed with a warning to the user (`softened` in `provalot stats`). Claims are matched by patterns and can miss paraphrases. Test outcomes are inferred from runner output (pytest, cargo, jest, vitest, node:test, go, xcodebuild, swift) because neither harness passes exit codes to hooks. A project's own test entry point counts too — a test-named script (`tools/test_service.sh`, `./run_tests.py`), `make test`, `./gradlew test`, `rspec`, `tox`, … — judged by its exit status plus any `FAIL`/`N failed` marker in its output.
