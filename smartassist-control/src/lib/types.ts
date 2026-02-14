// SmartAssist configuration types.
// Mirrors the Rust config schema in smartassist-core.

// ── Top-level ──────────────────────────────────────────────────

export interface Config {
  agents: AgentsConfig;
  channels: ChannelsConfig;
  gateway: GatewayConfig;
  session: SessionConfig;
  security: SecurityConfig;
  memory: MemoryConfig;
  logging: LoggingConfig;
  routing: RoutingConfig;
}

// ── Agents ─────────────────────────────────────────────────────

export interface AgentsConfig {
  default?: string;
  agents: Record<string, AgentConfig>;
  defaults: AgentDefaults;
}

export interface AgentDefaults {
  model?: string;
  models: Record<string, string>;
  thinking_level: ThinkingLevel;
  cli_backends: Record<string, string>;
  tools: ToolPolicyConfig;
  cache?: CacheConfig;
}

export interface AgentConfig {
  id: string;
  name?: string;
  workspace_dir?: string;
  model?: string;
  fallback_models: string[];
  system_prompt?: string;
  thinking_level: ThinkingLevel;
  tools: ToolPolicyConfig;
  sandbox?: SandboxConfig;
  subagents: SubagentConfig;
  identity?: AgentIdentity;
}

export type ThinkingLevel =
  | "off"
  | "minimal"
  | "low"
  | "medium"
  | "high"
  | "xhigh";

export interface ToolPolicyConfig {
  profile: ToolProfile;
  allow: string[];
  deny: string[];
  also_allow: string[];
}

export type ToolProfile = "minimal" | "coding" | "messaging" | "full";

export interface SandboxConfig {
  enabled: boolean;
  profile: SandboxProfile;
  container_name?: string;
  container_workdir?: string;
  limits: ResourceLimits;
}

export type SandboxProfile = "strict" | "standard" | "trusted" | "none";

export interface ResourceLimits {
  max_cpu_seconds: number;
  max_memory_bytes: number;
  max_processes: number;
  max_open_files: number;
  max_output_bytes: number;
}

export interface SubagentConfig {
  allow_agents: string[];
  model?: string;
  thinking?: ThinkingLevel;
  tool_policy?: ToolPolicyConfig;
}

export interface AgentIdentity {
  name: string;
  emoji?: string;
  avatar_url?: string;
  theme_color?: string;
}

export interface CacheConfig {
  type: CacheType;
  ttl_minutes: number;
}

export type CacheType = "prompt" | "static";

// ── Channels ───────────────────────────────────────────────────

export interface ChannelsConfig {
  telegram?: TelegramConfig;
  discord?: DiscordConfig;
  slack?: SlackConfig;
  signal?: SignalConfig;
  whatsapp?: WhatsAppConfig;
  extensions: string[];
}

export interface TelegramConfig {
  enabled: boolean;
  accounts: Record<string, TelegramAccountConfig>;
}

export interface TelegramAccountConfig {
  bot_token: string; // SecretString - displayed as [REDACTED]
  username?: string;
  webhook_url?: string;
  enabled: boolean;
}

export interface DiscordConfig {
  enabled: boolean;
  accounts: Record<string, DiscordAccountConfig>;
}

export interface DiscordAccountConfig {
  bot_token: string; // SecretString
  application_id?: string;
  enabled: boolean;
}

export interface SlackConfig {
  enabled: boolean;
  accounts: Record<string, SlackAccountConfig>;
}

export interface SlackAccountConfig {
  bot_token: string; // SecretString
  app_token?: string; // SecretString
  enabled: boolean;
}

export interface SignalConfig {
  enabled: boolean;
  api_url?: string;
  phone_number?: string;
}

export interface WhatsAppConfig {
  enabled: boolean;
  accounts: Record<string, WhatsAppAccountConfig>;
}

export interface WhatsAppAccountConfig {
  phone_number: string;
  enabled: boolean;
}

// ── Gateway ────────────────────────────────────────────────────

export interface GatewayConfig {
  bind: BindMode;
  port: number;
  control_ui: ControlUiConfig;
  http: HttpConfig;
  tailscale: TailscaleConfig;
}

export type BindMode = "loopback" | "lan" | "tailnet" | "auto";

export interface ControlUiConfig {
  enabled: boolean;
  auth: ControlUiAuth;
}

export interface ControlUiAuth {
  mode: ControlUiAuthMode;
  password?: string; // SecretString
  token?: string; // SecretString
}

export type ControlUiAuthMode = "identity" | "password" | "token";

export interface HttpConfig {
  endpoints: HttpEndpoints;
}

export interface HttpEndpoints {
  chat_completions: HttpEndpointConfig;
}

export interface HttpEndpointConfig {
  enabled: boolean;
}

export interface TailscaleConfig {
  mode: TailscaleMode;
}

export type TailscaleMode = "off" | "serve" | "funnel";

// ── Session ────────────────────────────────────────────────────

export interface SessionConfig {
  scope: SessionScope;
  dm_scope: DmScope;
  reset: SessionResetConfig;
}

export type SessionScope = "per_sender" | "global";

export type DmScope =
  | "main"
  | "per_peer"
  | "per_channel_peer"
  | "per_account_channel_peer";

export interface SessionResetConfig {
  mode: SessionResetMode;
  at_hour: number;
  idle_minutes: number;
}

export type SessionResetMode = "daily" | "idle" | "never";

// ── Security ───────────────────────────────────────────────────

export interface SecurityConfig {
  audit: AuditConfig;
  exec: ExecSecurityConfig;
  dm_policy: DmPolicy;
  sandbox: SecuritySandboxConfig;
}

export interface AuditConfig {
  enabled: boolean;
  log_path?: string;
  events: AuditEventFilter;
}

export interface AuditEventFilter {
  exec: boolean;
  auth: boolean;
  channel: boolean;
  security: boolean;
  config: boolean;
  session: boolean;
  agent: boolean;
}

export interface ExecSecurityConfig {
  mode: ExecMode;
  ask: AskMode;
  allowlist: string[];
  safe_bins: string[];
  approval_timeout_secs: number;
  ask_fallback: AskFallback;
}

export type ExecMode = "deny" | "allowlist" | "full";
export type AskMode = "off" | "on_miss" | "always";
export type AskFallback = "deny" | "allow";
export type DmPolicy = "open" | "pairing" | "allowlist" | "blocked";

export interface SecuritySandboxConfig {
  default_profile: SandboxProfile;
  default_limits: ResourceLimits;
}

// ── Memory ─────────────────────────────────────────────────────

export interface MemoryConfig {
  provider: MemoryProvider;
  embeddings: EmbeddingsProvider;
  search: MemorySearchConfig;
}

export type MemoryProvider = "lancedb" | "vector_only";
export type EmbeddingsProvider = "openai" | "google";

export interface MemorySearchConfig {
  limit: number;
  top_k: number;
}

// ── Logging ────────────────────────────────────────────────────

export interface LoggingConfig {
  level: LogLevel;
  file?: string;
  diagnostics: DiagnosticsConfig;
}

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

export interface DiagnosticsConfig {
  enabled: boolean;
  flags: string[];
}

// ── Routing ────────────────────────────────────────────────────

export interface RoutingConfig {
  bindings: RouteBinding[];
}

export interface RouteBinding {
  agent_id: string;
  match_channel?: string;
  match_account?: string;
  match_peer?: string;
  match_guild?: string;
}
