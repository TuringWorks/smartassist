import { createSignal, createResource, Show, For, onCleanup } from "solid-js";
import { rpcCall } from "../lib/tauri";
import { PageHeader } from "../components/FormComponents";

interface LogEntry { timestamp: string; level: string; message: string; component?: string; }
interface LogsResponse { logs: LogEntry[]; count: number; }

export default function Logs() {
  const [lines, setLines] = createSignal(100);
  const [level, setLevel] = createSignal("info");
  const [logs, { refetch }] = createResource(
    () => ({ lines: lines(), level: level() }),
    (opts) => rpcCall<LogsResponse>("logs.tail", { lines: opts.lines, level: opts.level }),
  );

  const interval = setInterval(() => refetch(), 3000);
  onCleanup(() => clearInterval(interval));

  const levelColor = (lvl: string): string => {
    switch (lvl.toLowerCase()) {
      case "error": return "var(--accent-red)";
      case "warn": return "var(--accent-amber)";
      case "info": return "var(--accent-blue)";
      case "debug": return "var(--text-secondary)";
      case "trace": return "var(--text-tertiary)";
      default: return "var(--text-primary)";
    }
  };

  const selectStyle = { background: "var(--bg-surface)", color: "var(--text-primary)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", padding: "6px 10px", "font-size": "13px", "font-family": "var(--font-sans)" };
  const btnStyle = { background: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", padding: "6px 14px", "font-size": "13px", cursor: "pointer", "font-family": "var(--font-sans)", "font-weight": "500" };

  return (
    <div>
      <PageHeader title="Live Logs" subtitle="Real-time log stream from the gateway" />

      <div style={{ display: "flex", gap: "12px", "margin-bottom": "16px", "align-items": "center" }}>
        <label style={{ color: "var(--text-secondary)", "font-size": "13px", display: "flex", "align-items": "center", gap: "6px" }}>
          Level:
          <select value={level()} onChange={(e) => setLevel(e.currentTarget.value)} style={selectStyle}>
            <option value="trace">Trace</option><option value="debug">Debug</option><option value="info">Info</option><option value="warn">Warn</option><option value="error">Error</option>
          </select>
        </label>
        <label style={{ color: "var(--text-secondary)", "font-size": "13px", display: "flex", "align-items": "center", gap: "6px" }}>
          Lines:
          <select value={lines()} onChange={(e) => setLines(Number(e.currentTarget.value))} style={selectStyle}>
            <option value={50}>50</option><option value={100}>100</option><option value={200}>200</option><option value={500}>500</option>
          </select>
        </label>
        <button onClick={() => refetch()} style={btnStyle}>Refresh</button>
      </div>

      <Show when={!logs.error} fallback={<div style={{ background: "var(--accent-red-dim)", border: "1px solid var(--accent-red)", "border-radius": "var(--radius-sm)", padding: "12px", color: "var(--accent-red)", "font-size": "13px" }}>{String(logs.error)}</div>}>
        <Show when={logs()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Connecting to gateway...</p>}>
          {(data) => (
            <div style={{ background: "var(--bg-input)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-md)", padding: "14px", "max-height": "calc(100vh - 240px)", "overflow-y": "auto", "font-family": "var(--font-mono)", "font-size": "12px", "line-height": "1.7" }}>
              <Show when={data().logs.length > 0} fallback={<p style={{ color: "var(--text-tertiary)", "font-style": "italic" }}>No log entries</p>}>
                <For each={data().logs}>
                  {(entry) => (
                    <div style={{ display: "flex", gap: "10px", padding: "3px 0", "border-bottom": "1px solid var(--border-subtle)" }}>
                      <span style={{ color: "var(--text-tertiary)", "white-space": "nowrap" }}>{entry.timestamp}</span>
                      <span style={{ color: levelColor(entry.level), "min-width": "48px", "font-weight": "600", "text-transform": "uppercase" }}>{entry.level}</span>
                      <Show when={entry.component}><span style={{ color: "var(--accent-cyan)" }}>[{entry.component}]</span></Show>
                      <span style={{ color: "var(--text-primary)" }}>{entry.message}</span>
                    </div>
                  )}
                </For>
              </Show>
            </div>
          )}
        </Show>
      </Show>
    </div>
  );
}
