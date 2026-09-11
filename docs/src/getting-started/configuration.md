# Configuration

SmartAssist is configured primarily through environment variables, with
per-session overrides available through CLI flags and the gateway's
`config.*` RPC methods (see [Gateway RPC Methods](../reference/gateway-rpc.md)).

## Environment variables

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Anthropic Claude API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `GOOGLE_API_KEY` / `GEMINI_API_KEY` | Google Gemini API key |
| `SMARTASSIST_PROVIDER` | Default model provider (`anthropic`, `openai`, `google`) |
| `SMARTASSIST_MODEL` | Default model to use |

## Model providers

See [Model Providers](../architecture/providers.md) for the full provider
specification, including streaming, tool calling, and vision support per
provider.

## Secrets

Credentials and tokens (channel bot tokens, provider keys) can be stored using
`smartassist-secrets`, which provides encrypted-at-rest credential management
rather than relying solely on plaintext environment variables.
