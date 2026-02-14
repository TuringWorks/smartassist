import { createSignal, createResource, Show, For, onCleanup } from "solid-js";
import { rpcCall } from "../lib/tauri";

interface CronJob {
  id: string;
  schedule: string;
  description?: string;
  agent_id: string;
  prompt: string;
  enabled: boolean;
  next_run?: string;
  last_run?: string;
  run_count: number;
}

interface CronListResponse {
  jobs: CronJob[];
  count: number;
}

export default function Cron() {
  const [jobs, { refetch }] = createResource(() =>
    rpcCall<CronListResponse>("cron.list"),
  );
  const [acting, setActing] = createSignal<string | null>(null);
  const [showAdd, setShowAdd] = createSignal(false);
  const [newSchedule, setNewSchedule] = createSignal("");
  const [newAgentId, setNewAgentId] = createSignal("default");
  const [newPrompt, setNewPrompt] = createSignal("");
  const [newDescription, setNewDescription] = createSignal("");

  // Poll every 10 seconds
  const interval = setInterval(() => refetch(), 10000);
  onCleanup(() => clearInterval(interval));

  const handleRun = async (id: string) => {
    setActing(id);
    try {
      await rpcCall("cron.run", { id });
      refetch();
    } catch (e) {
      console.error("Failed to run job:", e);
    }
    setActing(null);
  };

  const handleRemove = async (id: string) => {
    setActing(id);
    try {
      await rpcCall("cron.remove", { id });
      refetch();
    } catch (e) {
      console.error("Failed to remove job:", e);
    }
    setActing(null);
  };

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await rpcCall("cron.update", { id, enabled });
      refetch();
    } catch (e) {
      console.error("Failed to toggle job:", e);
    }
  };

  const handleAdd = async () => {
    if (!newSchedule() || !newPrompt()) return;
    setActing("add");
    try {
      await rpcCall("cron.add", {
        schedule: newSchedule(),
        agent_id: newAgentId(),
        prompt: newPrompt(),
        description: newDescription() || undefined,
        enabled: true,
      });
      setShowAdd(false);
      setNewSchedule("");
      setNewAgentId("default");
      setNewPrompt("");
      setNewDescription("");
      refetch();
    } catch (e) {
      console.error("Failed to add job:", e);
    }
    setActing(null);
  };

  const formatTime = (iso?: string): string => {
    if (!iso) return "-";
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  };

  const inputStyle = {
    background: "#0d1117",
    color: "#c9d1d9",
    border: "1px solid #30363d",
    "border-radius": "6px",
    padding: "8px 12px",
    "font-size": "13px",
    width: "100%",
    "box-sizing": "border-box" as const,
  };

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Cron Jobs
      </h1>

      <div
        style={{
          display: "flex",
          gap: "8px",
          "margin-bottom": "16px",
        }}
      >
        <button
          onClick={() => refetch()}
          style={{
            background: "#21262d",
            color: "#c9d1d9",
            border: "1px solid #30363d",
            "border-radius": "6px",
            padding: "4px 12px",
            "font-size": "13px",
            cursor: "pointer",
          }}
        >
          Refresh
        </button>
        <button
          onClick={() => setShowAdd(!showAdd())}
          style={{
            background: "#238636",
            color: "#fff",
            border: "1px solid #2ea043",
            "border-radius": "6px",
            padding: "4px 12px",
            "font-size": "13px",
            cursor: "pointer",
          }}
        >
          + Add Job
        </button>
      </div>

      {/* Add job form */}
      <Show when={showAdd()}>
        <div
          style={{
            background: "#161b22",
            border: "1px solid #30363d",
            "border-radius": "6px",
            padding: "16px",
            "margin-bottom": "16px",
          }}
        >
          <h3
            style={{
              color: "#f0f6fc",
              "font-size": "14px",
              "margin-bottom": "12px",
            }}
          >
            New Cron Job
          </h3>
          <div
            style={{ display: "grid", gap: "12px", "max-width": "500px" }}
          >
            <div>
              <label
                style={{
                  color: "#8b949e",
                  "font-size": "12px",
                  display: "block",
                  "margin-bottom": "4px",
                }}
              >
                Schedule (cron expression)
              </label>
              <input
                type="text"
                value={newSchedule()}
                onInput={(e) => setNewSchedule(e.currentTarget.value)}
                placeholder="0 0 * * * * *"
                style={inputStyle}
              />
              <p
                style={{
                  color: "#6e7681",
                  "font-size": "11px",
                  "margin-top": "4px",
                }}
              >
                Format: sec min hour day month weekday year
              </p>
            </div>
            <div>
              <label
                style={{
                  color: "#8b949e",
                  "font-size": "12px",
                  display: "block",
                  "margin-bottom": "4px",
                }}
              >
                Agent ID
              </label>
              <input
                type="text"
                value={newAgentId()}
                onInput={(e) => setNewAgentId(e.currentTarget.value)}
                placeholder="default"
                style={inputStyle}
              />
            </div>
            <div>
              <label
                style={{
                  color: "#8b949e",
                  "font-size": "12px",
                  display: "block",
                  "margin-bottom": "4px",
                }}
              >
                Prompt
              </label>
              <textarea
                value={newPrompt()}
                onInput={(e) => setNewPrompt(e.currentTarget.value)}
                placeholder="What should the agent do?"
                rows={3}
                style={{ ...inputStyle, resize: "vertical" as const }}
              />
            </div>
            <div>
              <label
                style={{
                  color: "#8b949e",
                  "font-size": "12px",
                  display: "block",
                  "margin-bottom": "4px",
                }}
              >
                Description (optional)
              </label>
              <input
                type="text"
                value={newDescription()}
                onInput={(e) => setNewDescription(e.currentTarget.value)}
                placeholder="What is this job for?"
                style={inputStyle}
              />
            </div>
            <div style={{ display: "flex", gap: "8px" }}>
              <button
                onClick={handleAdd}
                disabled={acting() === "add" || !newSchedule() || !newPrompt()}
                style={{
                  background: "#238636",
                  color: "#fff",
                  border: "1px solid #2ea043",
                  "border-radius": "6px",
                  padding: "6px 16px",
                  "font-size": "13px",
                  cursor: "pointer",
                  opacity:
                    acting() === "add" || !newSchedule() || !newPrompt()
                      ? "0.6"
                      : "1",
                }}
              >
                {acting() === "add" ? "Adding..." : "Create Job"}
              </button>
              <button
                onClick={() => setShowAdd(false)}
                style={{
                  background: "#21262d",
                  color: "#c9d1d9",
                  border: "1px solid #30363d",
                  "border-radius": "6px",
                  padding: "6px 16px",
                  "font-size": "13px",
                  cursor: "pointer",
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      </Show>

      <Show
        when={!jobs.error}
        fallback={
          <div
            style={{
              background: "#3d1f1f",
              border: "1px solid #f85149",
              "border-radius": "6px",
              padding: "12px",
              color: "#f85149",
              "font-size": "13px",
            }}
          >
            {String(jobs.error)}
          </div>
        }
      >
        <Show
          when={jobs()}
          fallback={
            <p style={{ color: "#8b949e" }}>Connecting to gateway...</p>
          }
        >
          {(data) => (
            <div>
              <p
                style={{
                  color: "#8b949e",
                  "font-size": "13px",
                  "margin-bottom": "12px",
                }}
              >
                {data().count} job{data().count !== 1 ? "s" : ""}
              </p>

              <Show
                when={data().jobs.length > 0}
                fallback={
                  <p style={{ color: "#8b949e", "font-style": "italic" }}>
                    No cron jobs configured
                  </p>
                }
              >
                <div style={{ display: "grid", gap: "8px" }}>
                  <For each={data().jobs}>
                    {(job) => (
                      <div
                        style={{
                          background: "#161b22",
                          border: "1px solid #30363d",
                          "border-radius": "6px",
                          padding: "16px",
                        }}
                      >
                        <div
                          style={{
                            display: "flex",
                            "justify-content": "space-between",
                            "align-items": "flex-start",
                            "margin-bottom": "8px",
                          }}
                        >
                          <div>
                            <div
                              style={{
                                display: "flex",
                                "align-items": "center",
                                gap: "8px",
                              }}
                            >
                              <span
                                style={{
                                  display: "inline-block",
                                  width: "8px",
                                  height: "8px",
                                  "border-radius": "50%",
                                  background: job.enabled
                                    ? "#3fb950"
                                    : "#8b949e",
                                }}
                              />
                              <strong style={{ color: "#f0f6fc" }}>
                                {job.description || job.id}
                              </strong>
                              <code
                                style={{
                                  color: "#a5d6ff",
                                  "font-size": "12px",
                                  background: "#0d1117",
                                  padding: "2px 6px",
                                  "border-radius": "4px",
                                }}
                              >
                                {job.schedule}
                              </code>
                            </div>
                          </div>

                          <div style={{ display: "flex", gap: "6px" }}>
                            <button
                              onClick={() =>
                                handleToggle(job.id, !job.enabled)
                              }
                              style={{
                                background: "#21262d",
                                color: "#c9d1d9",
                                border: "1px solid #30363d",
                                "border-radius": "6px",
                                padding: "4px 8px",
                                "font-size": "11px",
                                cursor: "pointer",
                              }}
                            >
                              {job.enabled ? "Disable" : "Enable"}
                            </button>
                            <button
                              onClick={() => handleRun(job.id)}
                              disabled={acting() === job.id}
                              style={{
                                background: "#21262d",
                                color: "#c9d1d9",
                                border: "1px solid #30363d",
                                "border-radius": "6px",
                                padding: "4px 8px",
                                "font-size": "11px",
                                cursor: "pointer",
                                opacity:
                                  acting() === job.id ? "0.6" : "1",
                              }}
                            >
                              Run Now
                            </button>
                            <button
                              onClick={() => handleRemove(job.id)}
                              disabled={acting() === job.id}
                              style={{
                                background: "#da3633",
                                color: "#fff",
                                border: "1px solid #f85149",
                                "border-radius": "6px",
                                padding: "4px 8px",
                                "font-size": "11px",
                                cursor: "pointer",
                                opacity:
                                  acting() === job.id ? "0.6" : "1",
                              }}
                            >
                              Remove
                            </button>
                          </div>
                        </div>

                        <div
                          style={{
                            "font-size": "13px",
                            color: "#8b949e",
                            display: "flex",
                            gap: "16px",
                            "flex-wrap": "wrap",
                          }}
                        >
                          <span>Agent: {job.agent_id}</span>
                          <span>Runs: {job.run_count}</span>
                          <span>Next: {formatTime(job.next_run)}</span>
                          <span>Last: {formatTime(job.last_run)}</span>
                        </div>

                        <div
                          style={{
                            "margin-top": "8px",
                            "font-size": "12px",
                            color: "#6e7681",
                            background: "#0d1117",
                            padding: "8px",
                            "border-radius": "4px",
                            "white-space": "pre-wrap",
                          }}
                        >
                          {job.prompt}
                        </div>
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
