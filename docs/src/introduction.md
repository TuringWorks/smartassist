# SmartAssist

High-performance Rust implementation of the SmartAssist AI agent gateway.

SmartAssist is a multi-channel AI agent platform: a single gateway process runs an
agent runtime with a large built-in tool library, exposes a JSON-RPC API over
WebSocket, and bridges out to messaging channels (Telegram, Discord, Slack,
Signal, WhatsApp, iMessage, LINE, and a browser-based web chat) while talking to
pluggable model providers (Anthropic, OpenAI, Google).

**Key metrics:** 167 source files, 725 tests passing, 101 agent tools, 46 gateway
RPC methods, 11 workspace crates.

## Workspace crates

| Crate | Description |
|-------|-------------|
| [`smartassist-core`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-core) | Core types, configuration, and shared utilities |
| [`smartassist-sandbox`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-sandbox) | Command execution sandboxing with platform-specific profiles |
| [`smartassist-channels`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-channels) | Messaging channel abstractions (Telegram, Discord, Slack, etc.) |
| [`smartassist-agent`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-agent) | Agent runtime with tool execution framework |
| [`smartassist-memory`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-memory) | Memory and context management for agents |
| [`smartassist-gateway`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-gateway) | JSON-RPC gateway server over WebSocket |
| [`smartassist-cli`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-cli) | Command-line interface |
| [`smartassist-plugin-sdk`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-plugin-sdk) | Plugin development kit for extensions |
| [`smartassist-providers`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-providers) | Model provider integrations (Anthropic, OpenAI, Google) |
| [`smartassist-secrets`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-secrets) | Encrypted secrets and credential management |
| [`smartassist-security`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-security) | Guardrails, sandboxing policy, and security tooling |
| [`smartassist-learnings`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-learnings) | Learnings/improvement engine for agent self-tuning |
| [`smartassist-browser`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-browser) | Browser automation |
| [`smartassist-canvas`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-canvas) | Canvas/UI surface support |
| [`smartassist-cron`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-cron) | Scheduled task execution |
| [`smartassist-transcription`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-transcription) | Audio transcription |
| [`smartassist-talk`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-talk) | Voice/talk interface |
| [`smartassist-mcp`](https://github.com/TuringWorks/smartassist/tree/main/crates/smartassist-mcp) | Model Context Protocol integration |

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│                        smartassist-cli                          │
│                    (Command-line interface)                     │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                      smartassist-gateway                        │
│              (JSON-RPC over WebSocket server)                   │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │  Health  │ │   Chat   │ │ Sessions │ │  Config  │  ...       │
│  │ Handler  │ │ Handler  │ │ Handler  │ │ Handler  │            │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘            │
└─────────────────────────────────────────────────────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          ▼                    ▼                    ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ smartassist-agent│  │smartassist-      │  │smartassist-memory│
│ (Agent runtime)  │  │  channels        │  │ (Context store)  │
│                  │  │  (Messaging)     │  │                  │
│  ┌────────────┐  │  │  ┌──────────┐    │  │  ┌────────────┐  │
│  │ Tools      │  │  │  │ Telegram │    │  │  │ Embeddings │  │
│  ├────────────┤  │  │  ├──────────┤    │  │  ├────────────┤  │
│  │ Sessions   │  │  │  │ Discord  │    │  │  │ Vector DB  │  │
│  ├────────────┤  │  │  ├──────────┤    │  │  └────────────┘  │
│  │ Streaming  │  │  │  │ Slack    │    │  │                  │
│  └────────────┘  │  │  ├──────────┤    │  └──────────────────┘
│                  │  │  │ Signal   │    │
│                  │  │  ├──────────┤    │
│                  │  │  │ WhatsApp │    │
│                  │  │  ├──────────┤    │
│                  │  │  │ Web +more│    │
│                  │  │  └──────────┘    │
└──────────────────┘  └──────────────────┘
          │
          ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│smartassist-      │  │smartassist-      │  │ smartassist-     │
│  sandbox         │  │  providers       │  │  secrets         │
│ (Cmd execution)  │  │ (AI models)      │  │ (Credentials)    │
└──────────────────┘  └──────────────────┘  └──────────────────┘
          │                    │                    │
          ▼                    ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       smartassist-core                          │
│                    (Types & configuration)                      │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation status

All four specification phases are complete:

| Phase | Focus | Status |
|-------|-------|--------|
| Phase 1 | Core Infrastructure (providers, gateway, tools) | Complete |
| Phase 2 | Channels (Telegram, Discord, Slack, Signal, WhatsApp, iMessage, LINE, Web) | Complete |
| Phase 3 | Advanced Features (Plugin SDK, browser automation, memory/embeddings) | Complete |
| Phase 4 | Platform & Polish (CLI, config validation, comprehensive testing) | Complete |

## Where to go next

- New to the project? Start with [Building](getting-started/building.md) and [Running](getting-started/running.md).
- Want the system design? See [Architecture Overview](architecture/overview.md).
- Looking for a specific tool or RPC method? See the [Reference](reference/crates.md) section.
