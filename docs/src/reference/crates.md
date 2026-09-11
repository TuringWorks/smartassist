# Repository Layout

```text
smartassist/
├── crates/
│   ├── smartassist-core           # Core types, configuration, shared utilities
│   ├── smartassist-sandbox        # Command execution sandboxing
│   ├── smartassist-channels       # Messaging channel implementations (feature-gated)
│   ├── smartassist-agent          # Agent runtime, sessions, and tool execution
│   ├── smartassist-memory         # Memory / vector store and embeddings
│   ├── smartassist-gateway        # JSON-RPC gateway server over WebSocket
│   ├── smartassist-cli            # Command-line interface
│   ├── smartassist-plugin-sdk     # Plugin development kit for extensions
│   ├── smartassist-providers      # Model provider integrations
│   ├── smartassist-secrets        # Encrypted secrets and credential management
│   ├── smartassist-security       # Guardrails and security policy
│   ├── smartassist-learnings      # Learnings / improvement engine
│   ├── smartassist-browser        # Browser automation
│   ├── smartassist-canvas         # Canvas / UI surface support
│   ├── smartassist-cron           # Scheduled task execution
│   ├── smartassist-transcription  # Audio transcription
│   ├── smartassist-talk           # Voice / talk interface
│   └── smartassist-mcp            # Model Context Protocol integration
├── channels/
│   └── smartassist-telegram       # Standalone Telegram channel binary
├── apps/
│   ├── android                    # Android client
│   └── ios                        # iOS client
├── smartassist-control/           # Tauri + Vite desktop control app
├── examples/
│   └── hello-plugin               # Example plugin using smartassist-plugin-sdk
├── tests/
│   ├── integration                # Integration test suite
│   ├── bdd                        # Cucumber BDD suite
│   ├── e2e                        # End-to-end suite
│   └── fixtures                   # Shared test fixtures
├── docs/                          # This documentation site (mdBook)
└── rust-spec/                     # Source specification documents
```

Each crate under `crates/` is a member of the root
[`Cargo.toml`](https://github.com/TuringWorks/smartassist/blob/main/Cargo.toml)
workspace, sharing a single `Cargo.lock` and dependency set defined under
`[workspace.dependencies]`.
