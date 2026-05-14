import { createSignal, createResource, Show, For, onCleanup } from "solid-js";
import { rpcCall } from "../lib/tauri";
import { PageHeader } from "../components/FormComponents";

interface CronJob { id: string; schedule: string; description?: string; agent_id: string; prompt: string; enabled: boolean; next_run?: string; last_run?: string; run_count: number; }
interface CronListResponse { jobs: CronJob[]; count: number; }

export default function Cron() {
  const [jobs, { refetch }] = createResource(() => rpcCall<CronListResponse>("cron.list"));
  const [acting, setActing] = createSignal<string | null>(null);
  const [showAdd, setShowAdd] = createSignal(false);
  const [newSchedule, setNewSchedule] = createSignal("");
  const [newAgentId, setNewAgentId] = createSignal("default");
  const [newPrompt, setNewPrompt] = createSignal("");
  const [newDescription, setNewDescription] = createSignal("");

  const interval = setInterval(() => refetch(), 10000);
  onCleanup(() => clearInterval(interval));

  const handleRun = async (id: string) => { setActing(id); try { await rpcCall("cron.run", { id }); refetch(); } catch (e) { console.error(e); } setActing(null); };
  const handleRemove = async (id: string) => { setActing(id); try { await rpcCall("cron.remove", { id }); refetch(); } catch (e) { console.error(e); } setActing(null); };
  const handleToggle = async (id: string, enabled: boolean) => { try { await rpcCall("cron.update", { id, enabled }); refetch(); } catch (e) { console.error(e); } };

  const handleAdd = async () => {
    if (!newSchedule() || !newPrompt()) return;
    setActing("add");
    try { await rpcCall("cron.add", { schedule: newSchedule(), agent_id: newAgentId(), prompt: newPrompt(), description: newDescription() || undefined, enabled: true }); setShowAdd(false); setNewSchedule(""); setNewAgentId("default"); setNewPrompt(""); setNewDescription(""); refetch(); } catch (e) { console.error(e); }
    setActing(null);
  };

  const formatTime = (iso?: string): string => { if (!iso) return "-"; try { return new Date(iso).toLocaleString(); } catch { return iso; } };

  const inputStyle = { background: "var(--bg-input)", color: "var(--text-primary)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", padding: "9px 14px", "font-size": "13px", width: "100%", "box-sizing": "border-box" as const, "font-family": "var(--font-sans)", outline: "none" };
  const btnGhost = { background: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", padding: "6px 14px", "font-size": "13px", cursor: "pointer", "font-family": "var(--font-sans)", "font-weight": "500" as const };
  const btnGreen = { background: "linear-gradient(135deg, #22c55e 0%, #16a34a 100%)", color: "#fff", border: "none", "border-radius": "var(--radius-sm)", padding: "6px 18px", "font-size": "13px", cursor: "pointer", "font-weight": "600" as const, "font-family": "var(--font-sans)" };
  const btnSmall = { background: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", padding: "4px 10px", "font-size": "11px", cursor: "pointer", "font-family": "var(--font-sans)" };
  const btnDel = { background: "linear-gradient(135deg, #ef4444 0%, #dc2626 100%)", color: "#fff", border: "none", "border-radius": "var(--radius-sm)", padding: "4px 10px", "font-size": "11px", cursor: "pointer", "font-family": "var(--font-sans)", "font-weight": "600" as const };

  return (
    <div>
      <PageHeader title="Cron Jobs" subtitle="Schedule recurring tasks for agents" />

      <div style={{ display: "flex", gap: "8px", "margin-bottom": "16px" }}>
        <button onClick={() => refetch()} style={btnGhost}>Refresh</button>
        <button onClick={() => setShowAdd(!showAdd())} style={btnGreen}>+ Add Job</button>
      </div>

      <Show when={showAdd()}>
        <div style={{ background: "var(--bg-card)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-md)", padding: "20px", "margin-bottom": "18px" }}>
          <h3 style={{ color: "var(--text-heading)", "font-size": "15px", "margin-bottom": "14px", "font-weight": "650" }}>New Cron Job</h3>
          <div style={{ display: "grid", gap: "14px", "max-width": "500px" }}>
            <div>
              <label style={{ color: "var(--text-secondary)", "font-size": "12px", display: "block", "margin-bottom": "4px", "font-weight": "600" }}>Schedule (cron expression)</label>
              <input type="text" value={newSchedule()} onInput={(e) => setNewSchedule(e.currentTarget.value)} placeholder="0 0 * * * * *" style={inputStyle} />
              <p style={{ color: "var(--text-tertiary)", "font-size": "11px", "margin-top": "4px" }}>Format: sec min hour day month weekday year</p>
            </div>
            <div>
              <label style={{ color: "var(--text-secondary)", "font-size": "12px", display: "block", "margin-bottom": "4px", "font-weight": "600" }}>Agent ID</label>
              <input type="text" value={newAgentId()} onInput={(e) => setNewAgentId(e.currentTarget.value)} placeholder="default" style={inputStyle} />
            </div>
            <div>
              <label style={{ color: "var(--text-secondary)", "font-size": "12px", display: "block", "margin-bottom": "4px", "font-weight": "600" }}>Prompt</label>
              <textarea value={newPrompt()} onInput={(e) => setNewPrompt(e.currentTarget.value)} placeholder="What should the agent do?" rows={3} style={{ ...inputStyle, resize: "vertical" as const }} />
            </div>
            <div>
              <label style={{ color: "var(--text-secondary)", "font-size": "12px", display: "block", "margin-bottom": "4px", "font-weight": "600" }}>Description (optional)</label>
              <input type="text" value={newDescription()} onInput={(e) => setNewDescription(e.currentTarget.value)} placeholder="What is this job for?" style={inputStyle} />
            </div>
            <div style={{ display: "flex", gap: "8px" }}>
              <button onClick={handleAdd} disabled={acting() === "add" || !newSchedule() || !newPrompt()} style={{ ...btnGreen, opacity: (acting() === "add" || !newSchedule() || !newPrompt()) ? "0.6" : "1" }}>
                {acting() === "add" ? "Adding..." : "Create Job"}
              </button>
              <button onClick={() => setShowAdd(false)} style={btnGhost}>Cancel</button>
            </div>
          </div>
        </div>
      </Show>

      <Show when={!jobs.error} fallback={<div style={{ background: "var(--accent-red-dim)", border: "1px solid var(--accent-red)", "border-radius": "var(--radius-sm)", padding: "12px", color: "var(--accent-red)", "font-size": "13px" }}>{String(jobs.error)}</div>}>
        <Show when={jobs()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Connecting to gateway...</p>}>
          {(data) => (
            <div>
              <p style={{ color: "var(--text-secondary)", "font-size": "13px", "margin-bottom": "12px" }}>{data().count} job{data().count !== 1 ? "s" : ""}</p>
              <Show when={data().jobs.length > 0} fallback={<p style={{ color: "var(--text-tertiary)", "font-style": "italic" }}>No cron jobs configured</p>}>
                <div style={{ display: "grid", gap: "10px" }}>
                  <For each={data().jobs}>
                    {(job) => (
                      <div style={{ background: "var(--bg-card)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-md)", padding: "18px" }}>
                        <div style={{ display: "flex", "justify-content": "space-between", "align-items": "flex-start", "margin-bottom": "8px" }}>
                          <div style={{ display: "flex", "align-items": "center", gap: "10px" }}>
                            <span style={{ display: "inline-block", width: "8px", height: "8px", "border-radius": "50%", background: job.enabled ? "var(--accent-green)" : "var(--text-tertiary)", "box-shadow": job.enabled ? "0 0 6px rgba(52,211,153,0.4)" : "none" }} />
                            <strong style={{ color: "var(--text-heading)" }}>{job.description || job.id}</strong>
                            <code style={{ color: "var(--accent-cyan)", "font-size": "12px", background: "var(--bg-input)", padding: "2px 8px", "border-radius": "4px", "font-family": "var(--font-mono)" }}>{job.schedule}</code>
                          </div>
                          <div style={{ display: "flex", gap: "6px" }}>
                            <button onClick={() => handleToggle(job.id, !job.enabled)} style={btnSmall}>{job.enabled ? "Disable" : "Enable"}</button>
                            <button onClick={() => handleRun(job.id)} disabled={acting() === job.id} style={{ ...btnSmall, opacity: acting() === job.id ? "0.6" : "1" }}>Run Now</button>
                            <button onClick={() => handleRemove(job.id)} disabled={acting() === job.id} style={{ ...btnDel, opacity: acting() === job.id ? "0.6" : "1" }}>Remove</button>
                          </div>
                        </div>
                        <div style={{ "font-size": "13px", color: "var(--text-secondary)", display: "flex", gap: "16px", "flex-wrap": "wrap" }}>
                          <span>Agent: {job.agent_id}</span><span>Runs: {job.run_count}</span><span>Next: {formatTime(job.next_run)}</span><span>Last: {formatTime(job.last_run)}</span>
                        </div>
                        <div style={{ "margin-top": "10px", "font-size": "12px", color: "var(--text-tertiary)", background: "var(--bg-input)", padding: "10px", "border-radius": "var(--radius-sm)", "white-space": "pre-wrap", "font-family": "var(--font-mono)" }}>{job.prompt}</div>
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
