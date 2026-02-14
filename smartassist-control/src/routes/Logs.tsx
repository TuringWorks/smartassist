import { createSignal, createResource, Show, For, onCleanup } from "solid-js";
import { rpcCall } from "../lib/tauri";

interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
  component?: string;
}

interface LogsResponse {
  logs: LogEntry[];
  count: number;
}

export default function Logs() {
  const [lines, setLines] = createSignal(100);
  const [level, setLevel] = createSignal("info");
  const [logs, { refetch }] = createResource(
    () => ({ lines: lines(), level: level() }),
    (opts) =>
      rpcCall<LogsResponse>("logs.tail", {
        lines: opts.lines,
        level: opts.level,
      }),
  );

  // Poll every 3 seconds
  const interval = setInterval(() => refetch(), 3000);
  onCleanup(() => clearInterval(interval));

  const levelColor = (lvl: string): string => {
    switch (lvl.toLowerCase()) {
      case "error":
        return "#f85149";
      case "warn":
        return "#d29922";
      case "info":
        return "#58a6ff";
      case "debug":
        return "#8b949e";
      case "trace":
        return "#6e7681";
      default:
        return "#c9d1d9";
    }
  };

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Live Logs
      </h1>

      <div
        style={{
          display: "flex",
          gap: "12px",
          "margin-bottom": "16px",
          "align-items": "center",
        }}
      >
        <label style={{ color: "#8b949e", "font-size": "13px" }}>
          Level:
          <select
            value={level()}
            onChange={(e) => setLevel(e.currentTarget.value)}
            style={{
              "margin-left": "8px",
              background: "#21262d",
              color: "#c9d1d9",
              border: "1px solid #30363d",
              "border-radius": "6px",
              padding: "4px 8px",
              "font-size": "13px",
            }}
          >
            <option value="trace">Trace</option>
            <option value="debug">Debug</option>
            <option value="info">Info</option>
            <option value="warn">Warn</option>
            <option value="error">Error</option>
          </select>
        </label>

        <label style={{ color: "#8b949e", "font-size": "13px" }}>
          Lines:
          <select
            value={lines()}
            onChange={(e) => setLines(Number(e.currentTarget.value))}
            style={{
              "margin-left": "8px",
              background: "#21262d",
              color: "#c9d1d9",
              border: "1px solid #30363d",
              "border-radius": "6px",
              padding: "4px 8px",
              "font-size": "13px",
            }}
          >
            <option value={50}>50</option>
            <option value={100}>100</option>
            <option value={200}>200</option>
            <option value={500}>500</option>
          </select>
        </label>

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
        when={!logs.error}
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
            {String(logs.error)}
          </div>
        }
      >
        <Show
          when={logs()}
          fallback={
            <p style={{ color: "#8b949e" }}>Connecting to gateway...</p>
          }
        >
          {(data) => (
            <div
              style={{
                background: "#0d1117",
                border: "1px solid #30363d",
                "border-radius": "6px",
                padding: "12px",
                "max-height": "calc(100vh - 240px)",
                "overflow-y": "auto",
                "font-family": "'SF Mono', 'Cascadia Code', monospace",
                "font-size": "12px",
                "line-height": "1.6",
              }}
            >
              <Show
                when={data().logs.length > 0}
                fallback={
                  <p style={{ color: "#8b949e", "font-style": "italic" }}>
                    No log entries
                  </p>
                }
              >
                <For each={data().logs}>
                  {(entry) => (
                    <div
                      style={{
                        display: "flex",
                        gap: "8px",
                        padding: "2px 0",
                        "border-bottom": "1px solid #21262d",
                      }}
                    >
                      <span
                        style={{ color: "#6e7681", "white-space": "nowrap" }}
                      >
                        {entry.timestamp}
                      </span>
                      <span
                        style={{
                          color: levelColor(entry.level),
                          "min-width": "48px",
                          "font-weight": "600",
                          "text-transform": "uppercase",
                        }}
                      >
                        {entry.level}
                      </span>
                      <Show when={entry.component}>
                        <span style={{ color: "#a5d6ff" }}>
                          [{entry.component}]
                        </span>
                      </Show>
                      <span style={{ color: "#c9d1d9" }}>
                        {entry.message}
                      </span>
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
