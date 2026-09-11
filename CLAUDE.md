# SmartAssist — Claude Code Guidelines

High-performance Rust implementation of the SmartAssist AI agent gateway: a
daemon (`smartassist-gateway`) running an agent runtime (`smartassist-agent`,
101 tools) that bridges 8 messaging channels and 3 model providers, plus a
Tauri/SolidJS desktop app (`smartassist-control`) and Android/iOS clients.

Full reference (idiom tables, refactor triggers, the "check before you write
a helper" table, known panic debt): [`AGENTS.md`](./AGENTS.md). Published
architecture/reference docs: [docs/](./docs/src/introduction.md) — also live at
https://turingworks.github.io/smartassist/.

## Quick Reference

### Build / test / run

```bash
cargo build --workspace                       # build everything
cargo build -p smartassist-cli                # CLI only
cargo test --workspace                        # unit + integration tests
cargo test --test cucumber                    # BDD suite (tests/bdd)
cargo fmt --all -- --check                    # matches CI
cargo clippy --workspace --all-targets --all-features -- -D warnings

ANTHROPIC_API_KEY=... cargo run -p smartassist-cli -- gateway run
```

Channel support in `smartassist-channels` is feature-gated
(`--features telegram,discord,slack,web,signal,imessage,whatsapp,line`) —
see [`docs/src/getting-started/building.md`](./docs/src/getting-started/building.md).

### Repo layout

18 crates under `crates/`, `smartassist-control/` (Tauri + SolidJS),
`apps/android` (Kotlin/Compose), `apps/ios` (Swift), `tests/{integration,bdd,e2e}`.
Full layout: [`docs/src/reference/crates.md`](./docs/src/reference/crates.md).

## Functional style & language idioms — Rust, TypeScript/SolidJS, Kotlin, Swift

Write new code, and leave touched code, toward: **pure functions over
immutable data, effects pushed to the edges, total error handling.**

- **Rust**: iterator combinators (`map`/`filter_map`/`fold`/`collect`) over
  index loops for data transforms; `for` loops are fine where the body is
  inherently effectful (task spawning, channel routing). `let` by default,
  `mut` only when forced. `?` + `map_err`/`ok_or_else` over `match` pyramids.
  **No new `.unwrap()`/`.expect()`/`panic!` on a fallible value in
  daemon/library/CLI-command paths** — tests and provably-safe invariants
  (with a one-line comment) are the exception. Borrow / `Arc` over `.clone()`
  in loops over sessions/channels/messages. Enums + exhaustive `match` over
  `bool` flags. `tokio::task::spawn_blocking` for anything blocking.
- **TypeScript/SolidJS** (`smartassist-control`): `const` and immutable
  updates; `createResource`/`createMemo` to derive state instead of a second
  `createSignal` synced by hand; route all backend calls through
  `src/lib/tauri.ts`'s wrappers or `rpcCall<T>`, never a bare `invoke()` in a
  route component; `unknown` + narrowing over `any`.
- **Kotlin** (`apps/android`): `val`/`data class` by default; `sealed
  interface` + exhaustive `when` for UI state instead of parallel
  `isLoading`/`error`/`data` fields; parse into a typed value once in
  `GatewayClient`, not inline in a `@Composable`; suspend functions for IO.
- **Swift** (`apps/ios`): `let`/`struct` by default; decode `Codable` types
  at the transport boundary; `enum` with associated values for UI state;
  gate iOS-only framework calls behind `#if os(iOS)` — the package also
  targets macOS.
- **Use the language's idiom, not the pattern's name.** Most GoF patterns are
  a language feature here: sum type → `enum`+`match` / discriminated
  union+`switch` / `sealed class`+`when`; strategy → a function value; RAII →
  `Drop`/`defer`/`use`. Refactor toward a pattern when the smell (a growing
  type-tag switch, a bool selecting behavior) is there — never because the
  pattern is admired.
- **Check for an existing helper before writing one.** Path-traversal guard →
  `smartassist_core::paths::is_within_workspace`; secret redaction →
  `smartassist_core::safety::leak_detector::mask_secret` /
  `smartassist_core::secret::SecretString`; hashing/ids →
  `smartassist_core::id::*`; sandboxed command execution →
  `smartassist_sandbox::executor::CommandExecutor`. Full table, including the
  gaps that don't have a canonical helper yet (retry/backoff, rate limiting,
  "is this on PATH"): [`AGENTS.md` → Check for an existing helper](./AGENTS.md#check-for-an-existing-helper-before-writing-one).

Full guidance + refactor triggers + known panic debt + per-crate error
handling convention: [`AGENTS.md` → Functional Style and Idioms](./AGENTS.md#functional-style-and-idioms-across-languages).

## Error handling

`smartassist-agent` and `smartassist-gateway` (and most crates) use a
`thiserror`-derived error enum per crate, propagated with `?`. `smartassist-cli`
uses `anyhow::Result` throughout since its errors are terminal. Match
whichever convention your crate already uses — see
[`AGENTS.md` → Error handling by crate type](./AGENTS.md#error-handling-by-crate-type).

## Testing

- `cargo test --workspace` — unit + integration tests per crate.
- `tests/bdd` (cucumber) — `cargo test --test cucumber`.
- `tests/e2e`, `tests/integration` — separate workspace members.
- CI (`.github/workflows/ci.yml`) gates on: fmt, clippy (`-D warnings`), test,
  BDD, a full-feature build, and coverage (tarpaulin → Codecov). All of it
  should be clean before a change is done.

## Discipline for refactors

Behavior-preserving only — pin with a test before refactoring under it. One
concern per commit (don't fold a style sweep into a feature/bug fix). Don't
claim a refactor is faster/safer without a test or measurement backing it.
