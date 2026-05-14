import { createSignal, createResource, Show, For, onCleanup } from "solid-js";
import { rpcCall } from "../lib/tauri";
import { PageHeader } from "../components/FormComponents";

interface Session { key: string; agent_id: string; status: string; created_at: string; last_activity: string; message_count: number; }
interface SessionsResponse { sessions: Session[]; total: number; }

export default function Sessions() {
  const [sessions, { refetch }] = createResource(() => rpcCall<SessionsResponse>("sessions.list", { limit: 100, offset: 0 }));
  const [deleting, setDeleting] = createSignal<string | null>(null);

  const interval = setInterval(() => refetch(), 5000);
  onCleanup(() => clearInterval(interval));

  const handleDelete = async (key: string) => {
    setDeleting(key);
    try { await rpcCall("sessions.delete", { session_key: key }); refetch(); } catch (e) { console.error("Failed to delete session:", e); }
    setDeleting(null);
  };

  const statusColor = (s: string): string => {
    switch (s) { case "active": return "var(--accent-green)"; case "paused": return "var(--accent-amber)"; case "archived": return "var(--text-tertiary)"; default: return "var(--text-primary)"; }
  };

  const formatTime = (iso: string): string => { try { return new Date(iso).toLocaleString(); } catch { return iso; } };

  const btnStyle = { background: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", padding: "6px 14px", "font-size": "13px", cursor: "pointer", "font-family": "var(--font-sans)", "font-weight": "500" as const };
  const btnDel = { background: "linear-gradient(135deg, #ef4444 0%, #dc2626 100%)", color: "#fff", border: "none", "border-radius": "var(--radius-sm)", padding: "6px 14px", "font-size": "12px", cursor: "pointer", "white-space": "nowrap" as const, "font-family": "var(--font-sans)", "font-weight": "600" as const };

  return (
    <div>
      <PageHeader title="Live Sessions" subtitle="Active conversation sessions across all channels" />

      <div style={{ "margin-bottom": "16px" }}>
        <button onClick={() => refetch()} style={btnStyle}>Refresh</button>
      </div>

      <Show when={!sessions.error} fallback={<div style={{ background: "var(--accent-red-dim)", border: "1px solid var(--accent-red)", "border-radius": "var(--radius-sm)", padding: "12px", color: "var(--accent-red)", "font-size": "13px" }}>{String(sessions.error)}</div>}>
        <Show when={sessions()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Connecting to gateway...</p>}>
          {(data) => (
            <div>
              <p style={{ color: "var(--text-secondary)", "font-size": "13px", "margin-bottom": "12px" }}>{data().total} session{data().total !== 1 ? "s" : ""}</p>
              <Show when={data().sessions.length > 0} fallback={<p style={{ color: "var(--text-tertiary)", "font-style": "italic" }}>No active sessions</p>}>
                <div style={{ display: "grid", gap: "10px" }}>
                  <For each={data().sessions}>
                    {(session) => (
                      <div style={{ background: "var(--bg-card)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-md)", padding: "18px", display: "flex", "justify-content": "space-between", "align-items": "flex-start" }}>
                        <div>
                          <div style={{ display: "flex", "align-items": "center", gap: "10px", "margin-bottom": "8px" }}>
                            <span style={{ display: "inline-block", width: "8px", height: "8px", "border-radius": "50%", background: statusColor(session.status), "box-shadow": session.status === "active" ? "0 0 6px rgba(52, 211, 153, 0.4)" : "none" }} />
                            <strong style={{ color: "var(--text-heading)" }}>{session.key}</strong>
                            <span style={{ color: statusColor(session.status), "font-size": "12px", "text-transform": "capitalize", "font-weight": "600" }}>{session.status}</span>
                          </div>
                          <div style={{ "font-size": "13px", color: "var(--text-secondary)", display: "flex", gap: "16px", "flex-wrap": "wrap" }}>
                            <span>Agent: {session.agent_id}</span>
                            <span>Messages: {session.message_count}</span>
                            <span>Created: {formatTime(session.created_at)}</span>
                            <span>Last: {formatTime(session.last_activity)}</span>
                          </div>
                        </div>
                        <button onClick={() => handleDelete(session.key)} disabled={deleting() === session.key} style={{ ...btnDel, opacity: deleting() === session.key ? "0.6" : "1" }}>
                          {deleting() === session.key ? "Deleting..." : "Delete"}
                        </button>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </div>
          )}
        </Show>
      </Show>
    </div>
  );
}
