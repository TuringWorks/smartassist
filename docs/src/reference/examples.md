# Quick Examples

Short, copy-pasteable snippets. For the full specifications, see
[Model Providers](../architecture/providers.md) and
[Plugin SDK](../architecture/plugins.md).

## Model providers

`smartassist-providers` includes integrations for major AI providers:

| Provider | Example models | Features |
|----------|-----------------|----------|
| Anthropic Claude | `claude-opus-4-20250514`, `claude-sonnet-4-20250514`, `claude-3-5-haiku-20241022` | Streaming, tool calling, vision, 200K context |
| OpenAI GPT | `gpt-4o`, `gpt-4o-mini`, `o1`, `o3-mini` | Streaming, tool calling, vision, 128K context |
| Google Gemini | `gemini-2.0-flash`, `gemini-1.5-pro` | Streaming, tool calling, vision, 2M context |

```rust
use smartassist_providers::{anthropic::AnthropicProvider, Provider};

let provider = AnthropicProvider::from_env()?;
let response = provider.chat(
    "claude-sonnet-4-20250514",
    &[Message::user("Hello!")],
    None,
).await?;
```

## Plugin SDK

Create custom plugins using the SDK:

```rust
use smartassist_plugin_sdk::prelude::*;

pub struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "my-plugin".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            description: "My custom plugin".to_string(),
            author: Some("Author".to_string()),
            homepage: None,
            license: Some("MIT".to_string()),
            capabilities: vec![PluginCapability::Tool],
            min_smartassist_version: None,
        }
    }

    async fn initialize(&mut self, ctx: &PluginContext) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}
```

Plugin capabilities:

- `Channel` - Custom messaging channels
- `Tool` - Custom agent tools
- `ModelProvider` - Custom AI model providers
- `Hook` - Middleware and interceptors
- `Storage` - Custom storage backends
- `Media` - Media processing

See [`examples/hello-plugin`](https://github.com/TuringWorks/smartassist/tree/main/examples/hello-plugin)
for a complete working plugin.
