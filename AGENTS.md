# SmartAssist — Agent & Contributor Guide

SmartAssist is a long-lived Rust daemon (`smartassist-gateway`) fronting an agent
runtime (`smartassist-agent`) with 101 tools, 8 messaging channels, and 3 model
providers, plus a Tauri/SolidJS desktop app and Android/iOS clients. This
document is the detailed reference; [`CLAUDE.md`](./CLAUDE.md) is the short
version Claude Code loads by default and links back here for the full story.

See also: [docs/](./docs/src/introduction.md) (the published architecture/reference
site) and [`README.md`](./README.md) (build/run quick start).

## Functional Style and Idioms Across Languages

SmartAssist ships four language surfaces: the Rust workspace (the daemon and
all business logic), `smartassist-control` (TypeScript + SolidJS, Tauri
desktop shell), `apps/android` (Kotlin + Jetpack Compose), and `apps/ios`
(Swift). Write new code, and leave touched code, closer to this style: **pure
functions over immutable data, effects pushed to the edges, total error
handling.**

**Guiding principle:** separate *computation* (pure, deterministic, testable
without a socket or a clock) from *effects* (IO, the gateway's WebSocket/HTTP
surface, channel APIs, the filesystem, subprocess execution). A tool
implementation that both computes a result and writes to disk is two
functions wearing a trenchcoat — the parsing/formatting/decision logic should
be a pure function under test; the `tokio::fs`/`Command`/reqwest call around
it is the one place that touches the outside world.

### The four rules, in priority order

1. **Pure by default.** Output depends only on input. Push IO, clocks, RNG,
   and mutation to the edges; keep the core a tree of pure transforms.
2. **Total functions.** Handle every case. Model absence and failure in the
   type — `Result`/`Option` in Rust, a discriminated union in TypeScript, a
   `sealed class`/`when` in Kotlin, an `enum` with associated values in Swift.
3. **Illegal states unrepresentable.** Encode invariants in types so the
   compiler rejects bad states before runtime, rather than checking a `bool`
   flag or a string tag at every call site.
4. **Immutability by default.** Reserve mutation for a locally-owned
   accumulator inside an otherwise-pure function.

**Where to stop being pure:** measured hot paths — the gateway's message
dispatch loop, a streaming tokenizer/parser in `smartassist-agent`, anything
in `smartassist-sandbox` that's on the critical path of starting a sandboxed
process. An indexed loop with a reused buffer is fine there, with a comment
saying why. *Functional by default; imperative where measured.*

### Reach for the language's idiom, not the pattern's name

Most classic design patterns already exist as a language feature in every
stack this repo ships. Using the feature reads as native and is shorter than
naming the pattern.

| Intent | Rust | TypeScript / SolidJS | Kotlin (Android/Compose) | Swift (iOS) |
|---|---|---|---|---|
| Sum type / visitor | `enum` + exhaustive `match` | discriminated union + `switch` with a `never` default | `sealed class`/`sealed interface` + exhaustive `when` | `enum` w/ associated values + `switch` |
| Strategy | trait object, or a `fn`/closure value | a function passed as a prop/param | a function type, or `fun interface` | a closure property, or a protocol |
| Scope guard / RAII | `Drop` | a cleanup function returned from `createEffect`/`onCleanup` | `use { }` | `defer` |
| Builder | typestate builder (see `ConfigBuilder`, `smartassist-core/src/config/loader.rs`) | an options object | named args with defaults, or `apply { }` | a result builder, or `init` with defaults |
| Iterator pipeline | `Iterator` adapters (`map`/`filter`/`fold`/`collect`) | `map`/`filter`/`reduce`/`flatMap` | sequences, `map`/`filter`/`fold` | `map`/`filter`/`reduce`, `lazy` |
| Newtype | `struct DeviceId(String)` | a branded type | `@JvmInline value class` | `struct Id: RawRepresentable` |
| Absence / failure | `Option`/`Result` + `?` | `T \| undefined`, a `Result`-shaped union | nullable `T?` + `?.`, `Result<T>` | `Optional` + `guard let`, `throws` |
| Observation | a channel, or `watch` | SolidJS signal/`createResource` | `StateFlow` | `@Observable`/Combine |

**Refactor toward a pattern when the smell is there** — a growing `match`/
`switch` on a type tag, a `bool` parameter selecting behavior, construction
logic sprawling across call sites. **Never because the pattern is admired.**
A one-implementation `Strategy` or a one-branch `Factory` is indirection with
no payer.

### Rust

- **Prefer iterator combinators over manual loops.** `map`/`filter`/
  `filter_map`/`fold`/`collect`/`partition` express intent and remove
  off-by-one classes of bug outright. `smartassist-agent`'s tool
  implementations (e.g. `crates/smartassist-agent/src/tools/filesystem.rs`)
  already lean this way — match that style in new tool code. A `for` loop is
  fine when the body is inherently effectful (spawning tasks, routing to
  channel instances, as in `crates/smartassist-channels/src/manager.rs`) —
  that's imperative-at-the-edge, not a smell.
- **Total error handling — no panics in daemon/library/command paths.**
  Return `Result`/`Option` and propagate with `?`, `map_err`, `ok_or_else`,
  `and_then`, `unwrap_or_default`. `.unwrap()`/`.expect()` are for tests and
  for invariants that provably cannot fail — leave a one-line comment saying
  why. See [Known panic debt](#known-panic-debt) below for what's already in
  the codebase and how to treat it.
- **Immutable by default.** Start every binding as `let`; add `mut` only when
  the compiler forces it. Prefer building a new value (struct update syntax
  `..old`, `Vec::from_iter`) over mutating in place.
- **Borrow, don't clone.** Take `&str`/`&[T]`/`&T` (or `impl AsRef<str>`,
  `Cow<'_, str>`) in signatures instead of owned `String`/`Vec<T>`. Share with
  `Arc<T>` rather than deep-cloning across the gateway's fan-out to channels/
  sessions. Every `.clone()` inside a loop over sessions, channels, or
  messages is a refactor candidate.
- **Make illegal states unrepresentable.** Enums over `bool` flags and
  stringly-typed status; newtypes over bare `String`/`Uuid` where a domain id
  shouldn't be interchangeable with an arbitrary string. Exhaustive `match`
  (no catch-all `_`) on domain enums like `ChannelType`, `ThinkingLevel`,
  `SessionScope` so a new variant is a compile error at every call site.
- **Use `entry()` to avoid double lookups.** `map.entry(k).or_insert_with(..)`
  instead of `if map.contains_key(&k) { ... } else { ... }`. Build a
  `HashSet`/`HashMap` once instead of an O(n²) nested-loop membership test.
- **Don't block the async runtime.** Gateway handlers and channel adapters
  run on `tokio`; wrap blocking IO/CPU work in `tokio::task::spawn_blocking`
  rather than calling it inline.

### TypeScript / SolidJS (`smartassist-control`)

- **`const` by default; never mutate props or a signal's value in place.**
  Build new values with spread/`map`/`filter`. `useConfig()`
  (`smartassist-control/src/lib/useConfig.ts`) is the template: it derives
  `dirty` from comparing `config` against `original` rather than tracking a
  separate mutable flag.
- **Route everything through the existing edges.** `tauri.ts`
  (`smartassist-control/src/lib/tauri.ts`) is the only file that calls
  `invoke()` — every route component goes through its typed wrappers or the
  generic `rpcCall<T>(method, params)` passthrough for anything not yet
  wrapped. Don't call `invoke()` directly from a route component; add a
  wrapper to `tauri.ts` (or use `rpcCall` if it's a one-off gateway RPC).
- **Derive, don't duplicate state.** Use `createResource` for anything
  fetched from the backend and `createMemo` for a value computed from
  existing signals, instead of a second `createSignal` kept in sync by hand.
  `createStore` is available in SolidJS but unused here — if a component's
  state grows nested enough to need it, that's a legitimate reason to reach
  for it; don't add it preemptively.
- **Total types.** No `any` — use `unknown` and narrow. Model variants as
  discriminated unions and switch exhaustively.

### Kotlin (`apps/android`)

- **`val` over `var`; `data class` for payloads.** Model UI state as a
  `sealed interface`/`sealed class` with exhaustive `when`, rather than
  parallel `isLoading`/`error`/`data` fields on a screen's state holder.
- **Parse into a typed value once, at the network boundary.** `GatewayClient`
  (`apps/android/app/src/main/java/com/smartassist/network/GatewayClient.kt`)
  is that boundary for gateway RPC calls — build request bodies as
  `JsonElement`s there (not raw `Map<String, String>`, which doesn't satisfy
  `JsonObject`'s constructor) and decode responses into typed models before
  they reach a `@Composable`.
- **Sequences and `map`/`filter`/`fold` over index loops** for anything
  transforming a list before it reaches Compose UI.
- **Suspend functions for IO; nothing blocking on the main dispatcher.**
  Ktor calls in `GatewayClient` are already `suspend fun` wrapped in
  `withContext(Dispatchers.IO)` — keep new network/IO calls on that pattern.

### Swift (`apps/ios`)

- **`let` over `var`; `struct` over `class`** unless identity/reference
  semantics are actually needed.
- **Decode `Codable` value types at the boundary** — wherever the WebSocket
  client (Starscream-based) receives a frame, decode it into a typed value
  immediately rather than passing a loosely-typed payload inward.
- **`enum` with associated values for state** (`.idle`/`.loading`/
  `.loaded(Session)`/`.failed(Error)`) instead of parallel optional
  properties.
- **Guard platform-specific APIs behind `#if os(iOS)`.** The package targets
  both iOS and macOS (`Package.swift`); an iOS-only framework call (e.g.
  `AVAudioSession` category/options that don't exist on macOS) must be
  wrapped in a platform check rather than assumed to compile everywhere the
  package builds. `Sources/Services/VoiceService.swift` is the case to check
  before extending it further.

### Check for an existing helper before writing one

The single largest source of duplication is a helper written a second time
because the first one was a few directories away. Some of these already have
one canonical home — use it. Others don't yet (marked **no canonical helper**)
— if you're about to write one, put it in `smartassist-core` and fold in the
existing duplicates listed rather than adding a third copy.

| You need | Use | Don't |
|---|---|---|
| Is this path inside the workspace (path-traversal guard)? | `smartassist_core::paths::is_within_workspace` (`crates/smartassist-core/src/paths.rs:109`) | A local `canonicalize`-and-compare — `smartassist-security`'s `config_audit.rs:26,45` already drifted into its own copy; don't add a third. |
| Locating SmartAssist's config/data directories | `smartassist_core::paths::{base_dir, config_file, sessions_dir, agent_dir, ...}` (`crates/smartassist-core/src/paths.rs`) | Hand-rolling `dirs::home_dir().join(...)` at a new call site. |
| Redacting a secret for logs/UI | `smartassist_core::safety::leak_detector::LeakDetector::mask_secret` (`crates/smartassist-core/src/safety/leak_detector.rs:309`), or wrap the value in `smartassist_core::secret::SecretString` so `Debug`/`Display` redact automatically | Byte-slicing a secret string yourself, or a bespoke redacted wrapper — `smartassist-secrets`' `DecryptedSecret` (`crates/smartassist-secrets/src/types.rs`) is a second one already; new code should prefer `SecretString` unless there's a reason specific to that crate. |
| SHA-256 / a short content hash | `smartassist_core::id::sha256` / `short_hash` (`crates/smartassist-core/src/id.rs:24,31`) | Re-deriving the hex-encoded digest inline — `smartassist-agent`'s `tools/checksum.rs` and `tools/encoding.rs` already implement the same md5/sha1/sha256/sha512 dispatch twice; if you touch either, consolidate into one shared function instead of leaving a third copy. |
| A UUID, slug, or short id | `smartassist_core::id::{uuid, short_id, slug_id, timestamp_id}` (`crates/smartassist-core/src/id.rs`) | `uuid::Uuid::new_v4().to_string()` ad hoc. |
| Running a sandboxed / resource-limited command | `smartassist_sandbox::executor::{CommandExecutor, execute_simple}` (`crates/smartassist-sandbox/src/executor.rs`) | `tokio::process::Command`/`std::process::Command` directly for anything touching untrusted or user-configured input — several crates already do this ad hoc (`smartassist-agent/tools/git.rs`, `tools/process.rs`, `smartassist-channels/imessage.rs`, `signal.rs`, `smartassist-mcp/client.rs`, `smartassist-providers/tts_local.rs`); prefer the sandboxed executor for new call sites, and treat migrating an existing one as a welcome, separately-committed cleanup. |
| Checking a prompt/tool-output for injection or policy violations | `smartassist_core::safety::{Sanitizer, SafetyLayer, Validator}` (`crates/smartassist-core/src/safety/`) | A new regex/keyword check bolted onto a specific tool or channel. |
| Retry/backoff for a flaky call | **No canonical helper.** `smartassist-channels/delivery.rs`, `smartassist-agent/tasks/executor.rs`, and `smartassist-providers/credential_pool.rs` each implement their own. If you need a fourth, consider factoring one into `smartassist-core` instead. | Copying whichever one is nearest without checking whether the arithmetic actually matches your case. |
| Rate limiting / call-count limits | **No canonical helper.** `smartassist-gateway/server.rs` (atomic window counter), `smartassist-agent/tools/guardrail.rs` (per-tool call-count map), and `smartassist-providers/credential_pool.rs` each solve a slightly different version of this. | Assuming one of these is reusable as-is for a new limiter without checking its semantics (window vs. count vs. cooldown differ between them). |
| "Is this binary on PATH?" | **No canonical helper** (no `which` crate dependency). `smartassist-providers/tts_local.rs` currently shells out to the external `which` binary, which doesn't exist on Windows if this ever targets it. | Copying the `Command::new("which")` pattern into a new call site — resolve `PATH` in Rust (`std::env::var("PATH")` + `Path::join` per entry) instead, or add a small helper if a second caller needs this. |

### Known panic debt

These are real `.unwrap()`/`.expect()` sites on fallible values found in
non-test code as of this writing — not a backlog to clear in one pass, but
worth fixing opportunistically when you're already touching the file, and a
reason to think twice before adding a new one nearby:

- `crates/smartassist-providers/src/openai.rs:243,350,424` —
  `org.parse().unwrap()` on a user-configured organization string when
  building request headers. A malformed value panics the request path;
  should be `.map_err(...)?` into the provider's error type.
- `crates/smartassist-channels/src/telegram.rs:404` —
  `url.parse().unwrap()` on an attachment URL that can originate from
  external input.
- `crates/smartassist-agent/src/tasks/registry.rs` (multiple sites, e.g.
  lines 132, 173, 232) — `self.conn.lock().unwrap()` on a
  `Mutex<Connection>`. A panic while holding the lock poisons it, so every
  later caller panics too, not just the one that failed first.
- `crates/smartassist-core/src/safety/sanitizer.rs:72,77,82,87` and
  `leak_detector.rs:202` — `Regex::new(...).expect(...)` and an Aho-Corasick
  build `.expect(...)` run on *every* `Sanitizer::new()`/`LeakDetector::new()`
  call, not once. Beyond the panic risk on a future pattern edit, rebuilding
  these on every construction is wasted work if either type is constructed
  more than once per process — hoist the compiled matcher into a
  `std::sync::LazyLock` if that turns out to matter.

Policy going forward: no new `.unwrap()`/`.expect()`/`panic!` on a value that
can plausibly fail in a daemon, library, or CLI-command path. Tests and
compile-time-provable invariants (with a one-line comment saying why the
value can't be absent) are the only exception.

### Error handling by crate type

The codebase already has a consistent, two-shape convention — follow whichever
your crate already uses rather than introducing a third:

- **Daemon/library crates** (`smartassist-agent`, `smartassist-gateway`, and
  most others): a `thiserror`-derived error enum per crate (see
  `crates/smartassist-agent/src/error.rs`, `crates/smartassist-gateway/src/error.rs`)
  with `#[from]` conversions for the common upstream error types, propagated
  with `?`. This is the dominant pattern — new fallible functions in these
  crates should return `Result<T, ThatCrate'sError>` and add a variant rather
  than reaching for `anyhow`.
- **`smartassist-cli`**: no per-crate error enum; uses `anyhow::Result`
  throughout, including interactive wizard/REPL flows where a manual `match`
  on a prompt result is often clearer than `?`-chaining. This is appropriate
  for a binary crate whose errors are terminal (printed and exited, never
  matched on by a caller) — don't introduce a `CliError` enum unless a caller
  outside this crate needs to distinguish error variants.

### Refactor triggers (safe, high-value — do these when you see them)

| Smell | Refactor | Why |
|---|---|---|
| `.unwrap()`/`.expect()` on a fallible value in non-test code | `?` + the crate's error enum, or `unwrap_or_else`/`ok_or_else` | no panic |
| `.clone()` inside a loop over sessions/channels/messages | borrow, or `Arc::clone` | fewer allocations, especially under the gateway's fan-out |
| Nested-loop membership/lookup | build a `HashSet`/`HashMap` once | O(n²) → O(n) |
| `let mut v = Vec::new(); for … { v.push(...) }` | `.iter().map(...).collect()` / `filter_map` | clarity, no off-by-one risk |
| `match` pyramid on `Result`/`Option` | `?`, `map_err`, `and_then` | clarity |
| A `bool` pair encoding a state (e.g. `is_loading` + `is_error`) | an enum with exhaustive `match`/`when`/`switch` | the two can't disagree |
| A regex/matcher rebuilt on every call/construction | hoist into `std::sync::LazyLock` (Rust) or a module-level constant | avoids repeated compilation cost, see [Known panic debt](#known-panic-debt) |
| Blocking IO/CPU on the async runtime | `tokio::task::spawn_blocking` | keeps the gateway responsive |
| A second copy of a helper already in the table above | use the existing one, or consolidate both into it | one behavior instead of two that can drift apart |
| Direct `Command::new(...)` for a user/model-triggered command | `smartassist_sandbox::executor::CommandExecutor` | sandboxing/resource limits apply consistently |

### Discipline for refactors

- **Behaviour-preserving only.** A style/safety refactor must not change
  observable output. Pin behavior with a test first if one doesn't already
  exist, then refactor under it.
- **CI is the gate, not a suggestion.** `.github/workflows/ci.yml` runs
  `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo test --workspace`, the cucumber BDD
  suite, a full-feature build, and coverage. All of it should be clean
  locally before you consider a change done: `cargo fmt --all && cargo
  clippy --workspace --all-targets --all-features -- -D warnings && cargo
  test --workspace`.
- **One concern per commit.** Don't fold a style sweep into a feature or bug
  fix — it makes review and `git bisect` painful. A mechanical FP refactor
  (e.g. converting a loop to combinators) goes in its own commit.
- **Don't refactor what you can't test.** If a "this is cleaner/safer" claim
  can't be pinned by an existing or new test, it's a judgment call, not a
  mechanical one — say so in the PR description rather than asserting it as
  fact.
