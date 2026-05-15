import { invoke } from "@tauri-apps/api/core";

/** Load the current SmartAssist configuration. */
export async function getConfig(): Promise<Record<string, unknown>> {
  return invoke("get_config");
}

/** Save the full configuration. Validates before writing. */
export async function saveConfig(
  config: Record<string, unknown>,
): Promise<void> {
  return invoke("save_config", { config });
}

/** Validate a configuration without saving. Returns list of errors. */
export async function validateConfig(
  config: Record<string, unknown>,
): Promise<string[]> {
  return invoke("validate_config", { config });
}

/** Get the path to the config file. */
export async function getConfigPath(): Promise<string> {
  return invoke("get_config_path");
}

/** Reset config to defaults. */
export async function resetConfig(): Promise<void> {
  return invoke("reset_config");
}

/** Get the application version. */
export async function getVersion(): Promise<string> {
  return invoke("get_version");
}

// ── Gateway process management ───────────────────────────────

export interface GatewayStatus {
  running: boolean;
  pid: number | null;
  port: number;
  uptime_secs: number | null;
}

/** Get the current gateway process status. */
export async function gatewayStatus(): Promise<GatewayStatus> {
  return invoke("gateway_status");
}

/** Start the gateway subprocess. */
export async function gatewayStart(): Promise<void> {
  return invoke("gateway_start");
}

/** Stop the gateway subprocess. */
export async function gatewayStop(): Promise<void> {
  return invoke("gateway_stop");
}

/** Restart the gateway subprocess. */
export async function gatewayRestart(): Promise<void> {
  return invoke("gateway_restart");
}

// ── Gateway RPC (via WebSocket) ──────────────────────────────

/** Run a security audit and return the report. */
export interface AuditReport {
  audit_name: string;
  timestamp: string;
  findings: AuditFinding[];
  summary: Record<string, number>;
}

export interface AuditFinding {
  rule_id: string;
  title: string;
  description: string;
  severity: "info" | "low" | "medium" | "high" | "critical";
  path?: string;
  remediation?: string;
}

export async function runSecurityAudit(
  auditType: "all" | "exec_surface" | "config_symlink" | "dm_policy",
): Promise<AuditReport> {
  return invoke("run_security_audit", { auditType });
}

export async function rpcCall<T = unknown>(
  method: string,
  params?: Record<string, unknown>,
): Promise<T> {
  return invoke("rpc_call", { method, params });
}

// ── Secrets Management ─────────────────────────────────────────

/** List all stored secret names. */
export async function listSecrets(): Promise<string[]> {
  return invoke("list_secrets");
}

/** Set or overwrite a secret. */
export async function setSecret(name: string, value: string): Promise<void> {
  return invoke("set_secret", { name, value });
}

/** Delete a secret by name. */
export async function deleteSecret(name: string): Promise<void> {
  return invoke("delete_secret", { name });
}

