# Building

SmartAssist is a Cargo workspace. The pinned toolchain is declared in
[`rust-toolchain.toml`](https://github.com/TuringWorks/smartassist/blob/main/rust-toolchain.toml)
(minimum supported Rust version: 1.75).

```bash
# Build all crates
cargo build --workspace

# Build with release optimizations
cargo build --workspace --release

# Build the CLI only
cargo build -p smartassist-cli
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p smartassist-agent

# Run tests with specific channel features
cargo test -p smartassist-channels --features telegram
```

The workspace also has BDD (cucumber) and end-to-end suites under
[`tests/bdd`](https://github.com/TuringWorks/smartassist/tree/main/tests/bdd) and
[`tests/e2e`](https://github.com/TuringWorks/smartassist/tree/main/tests/e2e), and an
integration suite under
[`tests/integration`](https://github.com/TuringWorks/smartassist/tree/main/tests/integration).
CI runs `cargo test --test cucumber` for the BDD suite; see
[`.github/workflows/ci.yml`](https://github.com/TuringWorks/smartassist/blob/main/.github/workflows/ci.yml)
for the full pipeline (format, clippy, test, BDD, build, coverage).

## Channel features

Messaging channels are feature-gated on `smartassist-channels` to reduce compile
time and dependencies:

```bash
# Build with Telegram support
cargo build -p smartassist-channels --features telegram

# Build with Discord support
cargo build -p smartassist-channels --features discord

# Build with Slack support
cargo build -p smartassist-channels --features slack

# Build with WebSocket support
cargo build -p smartassist-channels --features web

# Build with Signal support
cargo build -p smartassist-channels --features signal

# Build with iMessage support (macOS only)
cargo build -p smartassist-channels --features imessage

# Build with WhatsApp support
cargo build -p smartassist-channels --features whatsapp

# Build with LINE support
cargo build -p smartassist-channels --features line

# Build with all channels
cargo build -p smartassist-channels --features "telegram,discord,slack,web,signal,imessage,whatsapp,line"
```

See [Channel Traits](../architecture/channels/index.md) for how channels plug into
the gateway, and the per-channel pages for protocol details.
