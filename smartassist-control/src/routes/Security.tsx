import { Show, createSignal, For } from "solid-js";
import { useConfig } from "../lib/useConfig";
import { runSecurityAudit, type AuditReport, type AuditFinding } from "../lib/tauri";
import type {
  ExecMode,
  AskMode,
  AskFallback,
  DmPolicy,
  SandboxProfile,
} from "../lib/types";
import {
  EnumSelect,
  NumberInput,
  Toggle,
  TextInput,
  Section,
  SaveBar,
  Toast,
} from "../components/FormComponents";

// ── Severity badge colors ─────────────────────────────────────────

const severityColor: Record<string, string> = {
  info: "#8b949e",
  low: "#58a6ff",
  medium: "#d29922",
  high: "#f85149",
  critical: "#da3633",
};

const severityBg: Record<string, string> = {
  info: "#21262d",
  low: "#121d2f",
  medium: "#1f1c10",
  high: "#2a0f0f",
  critical: "#2a0f0f",
};

// ── Array string editor (inline component) ─────────────────────

function StringArrayEditor(props: {
  items: string[];
  onChange: (items: string[]) => void;
  placeholder?: string;
}) {
  const [input, setInput] = createSignal("");

  const add = () => {
    const v = input().trim();
    if (v && !props.items.includes(v)) {
      props.onChange([...props.items, v]);
      setInput("");
    }
  };

  const remove = (idx: number) => {
    const next = [...props.items];
    next.splice(idx, 1);
    props.onChange(next);
  };

  return (
    <div>
      <div style={{ display: "flex", gap: "8px", "margin-bottom": "8px" }}>
        <input
          type="text"
          value={input()}
          placeholder={props.placeholder || "Add item..."}
          onInput={(e) => setInput(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          style={{
            flex: 1,
            padding: "8px 12px",
            background: "#0d1117",
            border: "1px solid #30363d",
            "border-radius": "6px",
            color: "#e1e4e8",
            "font-size": "14px",
            "font-family": "inherit",
            outline: "none",
          }}
        />
        <button
          onClick={add}
          style={{
            padding: "8px 16px",
            background: "#238636",
            color: "#fff",
            border: "none",
            "border-radius": "6px",
            "font-size": "14px",
            cursor: "pointer",
          }}
        >
          Add
        </button>
      </div>
      <Show when={props.items.length > 0}>
        <div
          style={{
            display: "flex",
            "flex-wrap": "wrap",
            gap: "6px",
          }}
        >
          <For each={props.items}>
            {(item, idx) => (
              <span
                style={{
                  display: "inline-flex",
                  "align-items": "center",
                  gap: "6px",
                  padding: "4px 10px",
                  background: "#21262d",
                  border: "1px solid #30363d",
                  "border-radius": "12px",
                  "font-size": "13px",
                  color: "#c9d1d9",
                }}
              >
                {item}
                <button
                  onClick={() => remove(idx())}
                  style={{
                    background: "none",
                    border: "none",
                    color: "#8b949e",
                    cursor: "pointer",
                    "font-size": "14px",
                    padding: 0,
                    "line-height": 1,
                  }}
                  title="Remove"
                >
                  ×
                </button>
              </span>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

// ── Audit finding card ──────────────────────────────────────────

function FindingCard(props: { finding: AuditFinding }) {
  const f = props.finding;
  return (
    <div
      style={{
        padding: "12px 16px",
        background: severityBg[f.severity] || "#161b22",
        border: `1px solid ${severityColor[f.severity] || "#30363d"}`,
        "border-radius": "6px",
        "margin-bottom": "8px",
      }}
    >
      <div
        style={{
          display: "flex",
          "align-items": "center",
          gap: "8px",
          "margin-bottom": "4px",
        }}
      >
        <span
          style={{
            "font-size": "11px",
            "font-weight": 600,
            "text-transform": "uppercase",
            color: severityColor[f.severity] || "#8b949e",
            background: "rgba(0,0,0,0.3)",
            padding: "2px 6px",
            "border-radius": "4px",
          }}
        >
          {f.severity}
        </span>
        <strong style={{ "font-size": "14px", color: "#f0f6fc" }}>
          {f.title}
        </strong>
      </div>
      <div style={{ "font-size": "13px", color: "#8b949e", "margin-bottom": "4px" }}>
        {f.description}
      </div>
      <Show when={f.path}>
        <div style={{ "font-size": "12px", color: "#58a6ff", "margin-bottom": "4px" }}>
          Path: {f.path}
        </div>
      </Show>
      <Show when={f.remediation}>
        <div style={{ "font-size": "12px", color: "#d29922" }}>
          Remediation: {f.remediation}
        </div>
      </Show>
    </div>
  );
}

// ── Main Security page ──────────────────────────────────────────

export default function Security() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  const [auditLoading, setAuditLoading] = createSignal(false);
  const [auditReport, setAuditReport] = createSignal<AuditReport | null>(null);
  const [auditError, setAuditError] = createSignal("");

  const runAudit = async (type: "all" | "exec_surface" | "config_symlink" | "dm_policy") => {
    setAuditLoading(true);
    setAuditError("");
    try {
      const report = await runSecurityAudit(type);
      setAuditReport(report);
    } catch (e: any) {
      setAuditError(e?.toString?.() || "Audit failed");
    } finally {
      setAuditLoading(false);
    }
  };

  return (
    <div style={{ "padding-bottom": "80px" }}>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Security
      </h1>

      <Show when={config()} fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}>
        {(cfg) => (
          <>
            {/* ── Execution Policy ─────────────────────────────── */}
            <Section title="Execution Policy">
              <EnumSelect<ExecMode>
                label="Execution Mode"
                value={cfg().security.exec.mode}
                options={[
                  { value: "deny", label: "Deny (block all)" },
                  { value: "allowlist", label: "Allowlist" },
                  { value: "full", label: "Full (allow all)" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.mode = v;
                    return c;
                  })
                }
                help="Controls whether agents can execute commands"
              />
              <EnumSelect<AskMode>
                label="Approval Mode"
                value={cfg().security.exec.ask}
                options={[
                  { value: "off", label: "Off (never ask)" },
                  { value: "on_miss", label: "On Miss (ask when not in allowlist)" },
                  { value: "always", label: "Always" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.ask = v;
                    return c;
                  })
                }
              />
              <EnumSelect<AskFallback>
                label="Approval Fallback"
                value={cfg().security.exec.ask_fallback}
                options={[
                  { value: "deny", label: "Deny" },
                  { value: "allow", label: "Allow" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.ask_fallback = v;
                    return c;
                  })
                }
                help="What to do when approval request times out"
              />
              <NumberInput
                label="Approval Timeout (seconds)"
                value={cfg().security.exec.approval_timeout_secs}
                min={10}
                max={600}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.approval_timeout_secs = v;
                    return c;
                  })
                }
              />
            </Section>

            {/* ── Command Allowlist ────────────────────────────── */}
            <Section title="Command Allowlist">
              <div style={{ "margin-bottom": "16px" }}>
                <div
                  style={{
                    "font-size": "13px",
                    "font-weight": 500,
                    color: "#c9d1d9",
                    "margin-bottom": "6px",
                  }}
                >
                  Allowed Commands
                </div>
                <StringArrayEditor
                  items={cfg().security.exec.allowlist}
                  onChange={(items) =>
                    updateConfig((c) => {
                      c.security.exec.allowlist = items;
                      return c;
                    })
                  }
                  placeholder="e.g. ls, cat, grep"
                />
              </div>
              <div>
                <div
                  style={{
                    "font-size": "13px",
                    "font-weight": 500,
                    color: "#c9d1d9",
                    "margin-bottom": "6px",
                  }}
                >
                  Safe Binaries
                </div>
                <StringArrayEditor
                  items={cfg().security.exec.safe_bins}
                  onChange={(items) =>
                    updateConfig((c) => {
                      c.security.exec.safe_bins = items;
                      return c;
                    })
                  }
                  placeholder="e.g. /usr/bin/git, /usr/bin/curl"
                />
              </div>
            </Section>

            {/* ── DM Policy ────────────────────────────────────── */}
            <Section title="DM Policy">
              <EnumSelect<DmPolicy>
                label="Direct Message Policy"
                value={cfg().security.dm_policy}
                options={[
                  { value: "open", label: "Open (allow all)" },
                  { value: "pairing", label: "Pairing (require pairing)" },
                  { value: "allowlist", label: "Allowlist" },
                  { value: "blocked", label: "Blocked" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.dm_policy = v;
                    return c;
                  })
                }
              />
            </Section>

            {/* ── Security Audit ───────────────────────────────── */}
            <Section title="Security Audit" defaultOpen={false}>
              <div
                style={{
                  display: "flex",
                  gap: "8px",
                  "flex-wrap": "wrap",
                  "margin-bottom": "16px",
                }}
              >
                <button
                  onClick={() => runAudit("all")}
                  disabled={auditLoading()}
                  style={{
                    padding: "8px 16px",
                    background: auditLoading() ? "#21262d" : "#238636",
                    color: "#fff",
                    border: "none",
                    "border-radius": "6px",
                    "font-size": "14px",
                    cursor: auditLoading() ? "not-allowed" : "pointer",
                    opacity: auditLoading() ? 0.6 : 1,
                  }}
                >
                  {auditLoading() ? "Running..." : "Run Full Audit"}
                </button>
                <button
                  onClick={() => runAudit("exec_surface")}
                  disabled={auditLoading()}
                  style={{
                    padding: "8px 16px",
                    background: "#21262d",
                    color: "#c9d1d9",
                    border: "1px solid #30363d",
                    "border-radius": "6px",
                    "font-size": "14px",
                    cursor: auditLoading() ? "not-allowed" : "pointer",
                  }}
                >
                  Exec Surface
                </button>
                <button
                  onClick={() => runAudit("config_symlink")}
                  disabled={auditLoading()}
                  style={{
                    padding: "8px 16px",
                    background: "#21262d",
                    color: "#c9d1d9",
                    border: "1px solid #30363d",
                    "border-radius": "6px",
                    "font-size": "14px",
                    cursor: auditLoading() ? "not-allowed" : "pointer",
                  }}
                >
                  Config Symlinks
                </button>
                <button
                  onClick={() => runAudit("dm_policy")}
                  disabled={auditLoading()}
                  style={{
                    padding: "8px 16px",
                    background: "#21262d",
                    color: "#c9d1d9",
                    border: "1px solid #30363d",
                    "border-radius": "6px",
                    "font-size": "14px",
                    cursor: auditLoading() ? "not-allowed" : "pointer",
                  }}
                >
                  DM Policy
                </button>
              </div>

              <Show when={auditError()}>
                <div
                  style={{
                    padding: "10px 14px",
                    background: "#2a0f0f",
                    border: "1px solid #da3633",
                    "border-radius": "6px",
                    color: "#f85149",
                    "font-size": "13px",
                    "margin-bottom": "12px",
                  }}
                >
                  {auditError()}
                </div>
              </Show>

              <Show when={auditReport()}>
                {(report) => {
                  const r = report();
                  const total = r.findings.length;
                  const counts = r.summary || {};
                  return (
                    <div>
                      <div
                        style={{
                          display: "flex",
                          gap: "12px",
                          "margin-bottom": "12px",
                          "flex-wrap": "wrap",
                        }}
                      >
                        <div
                          style={{
                            padding: "6px 12px",
                            background: "#161b22",
                            border: "1px solid #30363d",
                            "border-radius": "6px",
                            "font-size": "13px",
                            color: "#c9d1d9",
                          }}
                        >
                          <strong>{total}</strong> findings
                        </div>
                        <For each={Object.entries(counts)}>
                          {([sev, count]) => (
                            <div
                              style={{
                                padding: "6px 12px",
                                background: severityBg[sev] || "#161b22",
                                border: `1px solid ${severityColor[sev] || "#30363d"}`,
                                "border-radius": "6px",
                                "font-size": "13px",
                                color: severityColor[sev] || "#c9d1d9",
                              }}
                            >
                              <strong>{count as number}</strong> {sev}
                            </div>
                          )}
                        </For>
                      </div>
                      <div style={{ "max-height": "400px", "overflow-y": "auto" }}>
                        <For each={r.findings}>
                          {(finding) => <FindingCard finding={finding} />}
                        </For>
                      </div>
                    </div>
                  );
                }}
              </Show>
            </Section>

            {/* ── Audit Configuration ──────────────────────────── */}
            <Section title="Audit Configuration" defaultOpen={false}>
              <Toggle
                label="Enable Audit Logging"
                value={cfg().security.audit.enabled}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.audit.enabled = v;
                    return c;
                  })
                }
              />
              <TextInput
                label="Log Path"
                value={cfg().security.audit.log_path ?? ""}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.audit.log_path = v || undefined;
                    return c;
                  })
                }
                placeholder="~/.smartassist/audit/audit.log"
              />
              <Show when={cfg().security.audit.enabled}>
                <div style={{ "margin-top": "12px" }}>
                  <strong
                    style={{
                      "font-size": "13px",
                      color: "#c9d1d9",
                      display: "block",
                      "margin-bottom": "8px",
                    }}
                  >
                    Event Filters
                  </strong>
                  <Toggle
                    label="Exec events"
                    value={cfg().security.audit.events.exec}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.exec = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Auth events"
                    value={cfg().security.audit.events.auth}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.auth = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Channel events"
                    value={cfg().security.audit.events.channel}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.channel = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Security events"
                    value={cfg().security.audit.events.security}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.security = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Config events"
                    value={cfg().security.audit.events.config}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.config = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Session events"
                    value={cfg().security.audit.events.session}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.session = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Agent events"
                    value={cfg().security.audit.events.agent}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.agent = v;
                        return c;
                      })
                    }
                  />
                </div>
              </Show>
            </Section>

            {/* ── Sandbox Defaults ─────────────────────────────── */}
            <Section title="Sandbox Defaults" defaultOpen={false}>
              <EnumSelect<SandboxProfile>
                label="Default Profile"
                value={cfg().security.sandbox.default_profile}
                options={[
                  { value: "strict", label: "Strict" },
                  { value: "standard", label: "Standard" },
                  { value: "trusted", label: "Trusted" },
                  { value: "none", label: "None" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.sandbox.default_profile = v;
                    return c;
                  })
                }
              />
              <NumberInput
                label="Max CPU Seconds"
                value={cfg().security.sandbox.default_limits.max_cpu_seconds}
                min={1}
                max={300}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.sandbox.default_limits.max_cpu_seconds = v;
                    return c;
                  })
                }
              />
              <NumberInput
                label="Max Processes"
                value={cfg().security.sandbox.default_limits.max_processes}
                min={1}
                max={100}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.sandbox.default_limits.max_processes = v;
                    return c;
                  })
                }
              />
            </Section>
          </>
        )}
      </Show>

      <SaveBar dirty={dirty()} saving={saving()} onSave={save} onDiscard={discard} />
      <Show when={toast()}>
        {(t) => <Toast message={t().message} type={t().type} visible={true} />}
      </Show>
    </div>
  );
}
