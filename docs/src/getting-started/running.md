# Running

```bash
# Start the gateway with Anthropic (default)
ANTHROPIC_API_KEY=your-key cargo run -p smartassist-cli -- gateway run

# Start with OpenAI
OPENAI_API_KEY=your-key cargo run -p smartassist-cli -- gateway run --provider openai

# Start with Google Gemini
GOOGLE_API_KEY=your-key cargo run -p smartassist-cli -- gateway run --provider google

# Start on a specific port with a custom model
ANTHROPIC_API_KEY=xxx cargo run -p smartassist-cli -- gateway run --port 18789 --model claude-opus-4-20250514

# Show help
cargo run -p smartassist-cli -- --help
```

Once running, the gateway exposes a JSON-RPC API over WebSocket — see
[Gateway RPC Methods](../reference/gateway-rpc.md) for the full method list and
[Gateway Overview](../architecture/gateway.md) for the protocol design.

## Docker

A [`Dockerfile`](https://github.com/TuringWorks/smartassist/blob/main/Dockerfile) and
[`docker-compose.yml`](https://github.com/TuringWorks/smartassist/blob/main/docker-compose.yml)
are provided for containerized deployment.

## Desktop control app

[`smartassist-control`](https://github.com/TuringWorks/smartassist/tree/main/smartassist-control)
is a Tauri + Vite desktop application for controlling and monitoring a running
gateway.
