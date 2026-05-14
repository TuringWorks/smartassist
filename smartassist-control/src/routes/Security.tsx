import { Show, createSignal, For } from "solid-js";
import { useConfig } from "../lib/useConfig";
import { runSecurityAudit, type AuditReport, type AuditFinding } from "../lib/tauri";
import type { ExecMode, AskMode, AskFallback, DmPolicy, SandboxProfile } from "../lib/types";
import { EnumSelect, NumberInput, Toggle, TextInput, Section, SaveBar, Toast, PageHeader } from "../components/FormComponents";

const severityColor: Record<string, string> = { info: "var(--text-secondary)", low: "var(--accent-blue)", medium: "var(--accent-amber)", high: "var(--accent-red)", critical: "#dc2626" };
const severityBg: Record<string, string> = { info: "var(--bg-surface)", low: "var(--accent-blue-dim)", medium: "var(--accent-amber-dim)", high: "var(--accent-red-dim)", critical: "var(--accent-red-dim)" };

function StringArrayEditor(props: { items: string[]; onChange: (items: string[]) => void; placeholder?: string }) {
  const [input, setInput] = createSignal("");
  const add = () => { const v = input().trim(); if (v && !props.items.includes(v)) { props.onChange([...props.items, v]); setInput(""); } };
  const remove = (idx: number) => { const next = [...props.items]; next.splice(idx, 1); props.onChange(next); };

  return (
    <div>
      <div style={{ display: "flex", gap: "8px", "margin-bottom": "8px" }}>
        <input type="text" value={input()} placeholder={props.placeholder || "Add item..."} onInput={(e) => setInput(e.currentTarget.value)} onKeyDown={(e) => e.key === "Enter" && add()} style={{ flex: "1", padding: "9px 14px", background: "var(--bg-input)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", color: "var(--text-primary)", "font-size": "14px", "font-family": "var(--font-sans)", outline: "none" }} />
        <button onClick={add} style={{ padding: "9px 18px", background: "linear-gradient(135deg, #22c55e 0%, #16a34a 100%)", color: "#fff", border: "none", "border-radius": "var(--radius-sm)", "font-size": "13px", cursor: "pointer", "font-weight": "600", "font-family": "var(--font-sans)" }}>Add</button>
      </div>
      <Show when={props.items.length > 0}>
        <div style={{ display: "flex", "flex-wrap": "wrap", gap: "6px" }}>
          <For each={props.items}>
            {(item, idx) => (
              <span style={{ display: "inline-flex", "align-items": "center", gap: "6px", padding: "4px 12px", background: "var(--bg-surface)", border: "1px solid var(--border-primary)", "border-radius": "14px", "font-size": "13px", color: "var(--text-primary)" }}>
                {item}
                <button onClick={() => remove(idx())} style={{ background: "none", border: "none", color: "var(--text-tertiary)", cursor: "pointer", "font-size": "14px", padding: "0", "line-height": "1" }} title="Remove">×</button>
              </span>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

function FindingCard(props: { finding: AuditFinding }) {
  const f = props.finding;
  return (
    <div style={{ padding: "14px 18px", background: severityBg[f.severity] || "var(--bg-card)", border: `1px solid ${severityColor[f.severity] || "var(--border-primary)"}`, "border-radius": "var(--radius-sm)", "margin-bottom": "8px" }}>
      <div style={{ display: "flex", "align-items": "center", gap: "8px", "margin-bottom": "6px" }}>
        <span style={{ "font-size": "10px", "font-weight": "700", "text-transform": "uppercase", "letter-spacing": "0.05em", color: severityColor[f.severity] || "var(--text-secondary)", background: "rgba(0,0,0,0.25)", padding: "2px 8px", "border-radius": "4px" }}>{f.severity}</span>
        <strong style={{ "font-size": "14px", color: "var(--text-heading)" }}>{f.title}</strong>
      </div>
      <div style={{ "font-size": "13px", color: "var(--text-secondary)", "margin-bottom": "4px" }}>{f.description}</div>
      <Show when={f.path}><div style={{ "font-size": "12px", color: "var(--accent-blue)", "margin-bottom": "4px" }}>Path: {f.path}</div></Show>
      <Show when={f.remediation}><div style={{ "font-size": "12px", color: "var(--accent-amber)" }}>Remediation: {f.remediation}</div></Show>
    </div>
  );
}

export default function Security() {
  const { config, dirty, saving, toast, updateConfig, save, discard } = useConfig();
  const [auditLoading, setAuditLoading] = createSignal(false);
  const [auditReport, setAuditReport] = createSignal<AuditReport | null>(null);
  const [auditError, setAuditError] = createSignal("");

  const runAudit = async (type: "all" | "exec_surface" | "config_symlink" | "dm_policy") => {
    setAuditLoading(true); setAuditError("");
    try { setAuditReport(await runSecurityAudit(type)); } catch (e: any) { setAuditError(e?.toString?.() || "Audit failed"); }
    finally { setAuditLoading(false); }
  };

  const auditBtnBase = { padding: "9px 18px", "border-radius": "var(--radius-sm)", "font-size": "13px", cursor: "pointer", "font-weight": "600" as const, "font-family": "var(--font-sans)", border: "none" };

  return (
    <div style={{ "padding-bottom": "80px" }}>
      <PageHeader title="Security" subtitle="Execution policies, sandboxing, audit, and DM access control" />

      <Show when={config()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Execution Policy">
              <EnumSelect<ExecMode> label="Execution Mode" value={cfg().security.exec.mode} options={[{ value: "deny", label: "Deny (block all)" }, { value: "allowlist", label: "Allowlist" }, { value: "full", label: "Full (allow all)" }]} onChange={(v) => updateConfig((c) => { c.security.exec.mode = v; return c; })} help="Controls whether agents can execute commands" />
              <EnumSelect<AskMode> label="Approval Mode" value={cfg().security.exec.ask} options={[{ value: "off", label: "Off (never ask)" }, { value: "on_miss", label: "On Miss" }, { value: "always", label: "Always" }]} onChange={(v) => updateConfig((c) => { c.security.exec.ask = v; return c; })} />
              <EnumSelect<AskFallback> label="Approval Fallback" value={cfg().security.exec.ask_fallback} options={[{ value: "deny", label: "Deny" }, { value: "allow", label: "Allow" }]} onChange={(v) => updateConfig((c) => { c.security.exec.ask_fallback = v; return c; })} help="What to do when approval request times out" />
              <NumberInput label="Approval Timeout (seconds)" value={cfg().security.exec.approval_timeout_secs} min={10} max={600} onChange={(v) => updateConfig((c) => { c.security.exec.approval_timeout_secs = v; return c; })} />
            </Section>

            <Section title="Command Allowlist">
              <div style={{ "margin-bottom": "16px" }}>
                <div style={{ "font-size": "13px", "font-weight": "600", color: "var(--text-secondary)", "margin-bottom": "6px" }}>Allowed Commands</div>
                <StringArrayEditor items={cfg().security.exec.allowlist} onChange={(items) => updateConfig((c) => { c.security.exec.allowlist = items; return c; })} placeholder="e.g. ls, cat, grep" />
              </div>
              <div>
                <div style={{ "font-size": "13px", "font-weight": "600", color: "var(--text-secondary)", "margin-bottom": "6px" }}>Safe Binaries</div>
                <StringArrayEditor items={cfg().security.exec.safe_bins} onChange={(items) => updateConfig((c) => { c.security.exec.safe_bins = items; return c; })} placeholder="e.g. /usr/bin/git" />
              </div>
            </Section>

            <Section title="DM Policy">
              <EnumSelect<DmPolicy> label="Direct Message Policy" value={cfg().security.dm_policy} options={[{ value: "open", label: "Open (allow all)" }, { value: "pairing", label: "Pairing" }, { value: "allowlist", label: "Allowlist" }, { value: "blocked", label: "Blocked" }]} onChange={(v) => updateConfig((c) => { c.security.dm_policy = v; return c; })} />
            </Section>

            <Section title="Security Audit" defaultOpen={false}>
              <div style={{ display: "flex", gap: "8px", "flex-wrap": "wrap", "margin-bottom": "16px" }}>
                <button onClick={() => runAudit("all")} disabled={auditLoading()} style={{ ...auditBtnBase, background: auditLoading() ? "var(--bg-surface)" : "linear-gradient(135deg, #22c55e 0%, #16a34a 100%)", color: "#fff", opacity: auditLoading() ? "0.6" : "1" }}>{auditLoading() ? "Running..." : "Run Full Audit"}</button>
                <button onClick={() => runAudit("exec_surface")} disabled={auditLoading()} style={{ ...auditBtnBase, background: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-primary)" }}>Exec Surface</button>
                <button onClick={() => runAudit("config_symlink")} disabled={auditLoading()} style={{ ...auditBtnBase, background: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-primary)" }}>Config Symlinks</button>
                <button onClick={() => runAudit("dm_policy")} disabled={auditLoading()} style={{ ...auditBtnBase, background: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-primary)" }}>DM Policy</button>
              </div>
              <Show when={auditError()}><div style={{ padding: "12px 16px", background: "var(--accent-red-dim)", border: "1px solid var(--accent-red)", "border-radius": "var(--radius-sm)", color: "var(--accent-red)", "font-size": "13px", "margin-bottom": "12px" }}>{auditError()}</div></Show>
              <Show when={auditReport()}>
                {(report) => {
                  const r = report(); const total = r.findings.length; const counts = r.summary || {};
                  return (
                    <div>
                      <div style={{ display: "flex", gap: "10px", "margin-bottom": "14px", "flex-wrap": "wrap" }}>
                        <div style={{ padding: "6px 14px", background: "var(--bg-card)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", "font-size": "13px", color: "var(--text-primary)", "font-weight": "600" }}><strong>{total}</strong> findings</div>
                        <For each={Object.entries(counts)}>
                          {([sev, count]) => (<div style={{ padding: "6px 14px", background: severityBg[sev] || "var(--bg-card)", border: `1px solid ${severityColor[sev] || "var(--border-primary)"}`, "border-radius": "var(--radius-sm)", "font-size": "13px", color: severityColor[sev] || "var(--text-primary)" }}><strong>{count as number}</strong> {sev}</div>)}
                        </For>
                      </div>
                      <div style={{ "max-height": "400px", "overflow-y": "auto" }}>
                        <For each={r.findings}>{(finding) => <FindingCard finding={finding} />}</For>
                      </div>
                    </div>
                  );
                }}
              </Show>
            </Section>

            <Section title="Audit Configuration" defaultOpen={false}>
              <Toggle label="Enable Audit Logging" value={cfg().security.audit.enabled} onChange={(v) => updateConfig((c) => { c.security.audit.enabled = v; return c; })} />
              <TextInput label="Log Path" value={cfg().security.audit.log_path ?? ""} onChange={(v) => updateConfig((c) => { c.security.audit.log_path = v || undefined; return c; })} placeholder="~/.smartassist/audit/audit.log" />
              <Show when={cfg().security.audit.enabled}>
                <div style={{ "margin-top": "14px" }}>
                  <strong style={{ "font-size": "13px", color: "var(--text-secondary)", display: "block", "margin-bottom": "10px", "font-weight": "600" }}>Event Filters</strong>
                  <Toggle label="Exec events" value={cfg().security.audit.events.exec} onChange={(v) => updateConfig((c) => { c.security.audit.events.exec = v; return c; })} />
                  <Toggle label="Auth events" value={cfg().security.audit.events.auth} onChange={(v) => updateConfig((c) => { c.security.audit.events.auth = v; return c; })} />
                  <Toggle label="Channel events" value={cfg().security.audit.events.channel} onChange={(v) => updateConfig((c) => { c.security.audit.events.channel = v; return c; })} />
                  <Toggle label="Security events" value={cfg().security.audit.events.security} onChange={(v) => updateConfig((c) => { c.security.audit.events.security = v; return c; })} />
                  <Toggle label="Config events" value={cfg().security.audit.events.config} onChange={(v) => updateConfig((c) => { c.security.audit.events.config = v; return c; })} />
                  <Toggle label="Session events" value={cfg().security.audit.events.session} onChange={(v) => updateConfig((c) => { c.security.audit.events.session = v; return c; })} />
                  <Toggle label="Agent events" value={cfg().security.audit.events.agent} onChange={(v) => updateConfig((c) => { c.security.audit.events.agent = v; return c; })} />
                </div>
              </Show>
            </Section>

            <Section title="Sandbox Defaults" defaultOpen={false}>
              <EnumSelect<SandboxProfile> label="Default Profile" value={cfg().security.sandbox.default_profile} options={[{ value: "strict", label: "Strict" }, { value: "standard", label: "Standard" }, { value: "trusted", label: "Trusted" }, { value: "none", label: "None" }]} onChange={(v) => updateConfig((c) => { c.security.sandbox.default_profile = v; return c; })} />
              <NumberInput label="Max CPU Seconds" value={cfg().security.sandbox.default_limits.max_cpu_seconds} min={1} max={300} onChange={(v) => updateConfig((c) => { c.security.sandbox.default_limits.max_cpu_seconds = v; return c; })} />
              <NumberInput label="Max Processes" value={cfg().security.sandbox.default_limits.max_processes} min={1} max={100} onChange={(v) => updateConfig((c) => { c.security.sandbox.default_limits.max_processes = v; return c; })} />
            </Section>
          </>
        )}
      </Show>

      <SaveBar dirty={dirty()} saving={saving()} onSave={save} onDiscard={discard} />
      <Show when={toast()}>{(t) => <Toast message={t().message} type={t().type} visible={true} />}</Show>
    </div>
  );
}
