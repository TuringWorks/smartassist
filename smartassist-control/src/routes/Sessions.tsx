import { createSignal, createResource, Show, For, onCleanup } from "solid-js";
import { rpcCall } from "../lib/tauri";

interface Session {
  key: string;
  agent_id: string;
  status: string;
  created_at: string;
  last_activity: string;
  message_count: number;
}

interface SessionsResponse {
  sessions: Session[];
  total: number;
}

export default function Sessions() {
  const [sessions, { refetch }] = createResource(() =>
    rpcCall<SessionsResponse>("sessions.list", { limit: 100, offset: 0 }),
  );
  const [deleting, setDeleting] = createSignal<string | null>(null);

  // Poll every 5 seconds
  const interval = setInterval(() => refetch(), 5000);
  onCleanup(() => clearInterval(interval));

  const handleDelete = async (key: string) => {
    setDeleting(key);
    try {
      await rpcCall("sessions.delete", { session_key: key });
      refetch();
    } catch (e) {
      console.error("Failed to delete session:", e);
    }
    setDeleting(null);
  };

  const statusColor = (s: string): string => {
    switch (s) {
      case "active":
        return "#3fb950";
      case "paused":
        return "#d29922";
      case "archived":
        return "#8b949e";
      default:
        return "#c9d1d9";
    }
  };

  const formatTime = (iso: string): string => {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  };

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Live Sessions
      </h1>

      <div style={{ "margin-bottom": "16px" }}>
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
      </div>

      <Show
        when={!sessions.error}
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
            {String(sessions.error)}
          </div>
        }
      >
        <Show
          when={sessions()}
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
                {data().total} session{data().total !== 1 ? "s" : ""}
              </p>

              <Show
                when={data().sessions.length > 0}
                fallback={
                  <p style={{ color: "#8b949e", "font-style": "italic" }}>
                    No active sessions
                  </p>
                }
              >
                <div
                  style={{
                    display: "grid",
                    gap: "8px",
                  }}
                >
                  <For each={data().sessions}>
                    {(session) => (
                      <div
                        style={{
                          background: "#161b22",
                          border: "1px solid #30363d",
                          "border-radius": "6px",
                          padding: "16px",
                          display: "flex",
                          "justify-content": "space-between",
                          "align-items": "flex-start",
                        }}
                      >
                        <div>
                          <div
                            style={{
                              display: "flex",
                              "align-items": "center",
                              gap: "8px",
                              "margin-bottom": "8px",
                            }}
                          >
                            <span
                              style={{
                                display: "inline-block",
                                width: "8px",
                                height: "8px",
                                "border-radius": "50%",
                                background: statusColor(session.status),
                              }}
                            />
                            <strong style={{ color: "#f0f6fc" }}>
                              {session.key}
                            </strong>
                            <span
                              style={{
                                color: statusColor(session.status),
                                "font-size": "12px",
                                "text-transform": "capitalize",
                              }}
                            >
                              {session.status}
                            </span>
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
                            <span>Agent: {session.agent_id}</span>
                            <span>
                              Messages: {session.message_count}
                            </span>
                            <span>
                              Created: {formatTime(session.created_at)}
                            </span>
                            <span>
                              Last activity:{" "}
                              {formatTime(session.last_activity)}
                            </span>
                          </div>
                        </div>

                        <button
                          onClick={() => handleDelete(session.key)}
                          disabled={deleting() === session.key}
                          style={{
                            background: "#da3633",
                            color: "#fff",
                            border: "1px solid #f85149",
                            "border-radius": "6px",
                            padding: "4px 12px",
                            "font-size": "12px",
                            cursor: "pointer",
                            "white-space": "nowrap",
                            opacity:
                              deleting() === session.key ? "0.6" : "1",
                          }}
                        >
                          {deleting() === session.key
                            ? "Deleting..."
                            : "Delete"}
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
