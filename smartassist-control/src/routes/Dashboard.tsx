import { createResource, createSignal, Show } from "solid-js";
import {
  getConfig,
  getVersion,
  getConfigPath,
  gatewayStatus,
  gatewayStart,
  gatewayStop,
  gatewayRestart,
} from "../lib/tauri";
export default function Dashboard() {
  const [version] = createResource(getVersion);
  const [configPath] = createResource(getConfigPath);
  const [config] = createResource(getConfig);
  const [gwStatus, { refetch: refetchGw }] = createResource(gatewayStatus);
  const [gwAction, setGwAction] = createSignal<string | null>(null);

  const handleStart = async () => {
    setGwAction("starting");
    try {
      await gatewayStart();
    } catch (e) {
      console.error("Failed to start gateway:", e);
    }
    // Give the process a moment to start
    setTimeout(() => {
      refetchGw();
      setGwAction(null);
    }, 1000);
  };

  const handleStop = async () => {
    setGwAction("stopping");
    try {
      await gatewayStop();
    } catch (e) {
      console.error("Failed to stop gateway:", e);
    }
    setTimeout(() => {
      refetchGw();
      setGwAction(null);
    }, 500);
  };

  const handleRestart = async () => {
    setGwAction("restarting");
    try {
      await gatewayRestart();
    } catch (e) {
      console.error("Failed to restart gateway:", e);
    }
    setTimeout(() => {
      refetchGw();
      setGwAction(null);
    }, 1500);
  };

  const formatUptime = (secs: number): string => {
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  };

  // Poll gateway status every 5 seconds
  setInterval(() => refetchGw(), 5000);

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Dashboard
      </h1>

      <div
        style={{
          display: "grid",
          "grid-template-columns": "repeat(auto-fit, minmax(300px, 1fr))",
          gap: "16px",
        }}
      >
        {/* Version card */}
        <div class="card">
          <h3>SmartAssist</h3>
          <p style={{ "font-size": "14px", color: "#8b949e" }}>
            Version: {version() ?? "..."}
          </p>
          <p
            style={{
              "font-size": "12px",
              color: "#8b949e",
              "margin-top": "8px",
              "word-break": "break-all",
            }}
          >
            Config: {configPath() ?? "..."}
          </p>
        </div>

        {/* Gateway status card */}
        <div class="card">
          <h3>Gateway</h3>
          <Show
            when={gwStatus()}
            fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}
          >
            {(gw) => (
              <div>
                <div
                  style={{
                    display: "flex",
                    "align-items": "center",
                    gap: "8px",
                    "margin-bottom": "12px",
                  }}
                >
                  <span
                    style={{
                      display: "inline-block",
                      width: "10px",
                      height: "10px",
                      "border-radius": "50%",
                      background: gw().running ? "#3fb950" : "#8b949e",
                    }}
                  />
                  <span
                    style={{
                      "font-size": "14px",
                      color: gw().running ? "#3fb950" : "#8b949e",
                      "font-weight": "600",
                    }}
                  >
                    {gw().running ? "Running" : "Stopped"}
                  </span>
                </div>
                <p style={{ "font-size": "14px", color: "#8b949e" }}>
                  Port: {gw().port}
                </p>
                <Show when={gw().running && gw().pid}>
                  <p style={{ "font-size": "14px", color: "#8b949e" }}>
                    PID: {gw().pid}
                  </p>
                </Show>
                <Show when={gw().running && gw().uptime_secs != null}>
                  <p style={{ "font-size": "14px", color: "#8b949e" }}>
                    Uptime: {formatUptime(gw().uptime_secs!)}
                  </p>
                </Show>

                <div
                  style={{
                    display: "flex",
                    gap: "8px",
                    "margin-top": "16px",
                  }}
                >
                  <Show when={!gw().running}>
                    <button
                      class="btn-primary"
                      disabled={gwAction() !== null}
                      onClick={handleStart}
                    >
                      {gwAction() === "starting" ? "Starting..." : "Start"}
                    </button>
                  </Show>
                  <Show when={gw().running}>
                    <button
                      class="btn-danger"
                      disabled={gwAction() !== null}
                      onClick={handleStop}
                    >
                      {gwAction() === "stopping" ? "Stopping..." : "Stop"}
                    </button>
                    <button
                      class="btn-secondary"
                      disabled={gwAction() !== null}
                      onClick={handleRestart}
                    >
                      {gwAction() === "restarting"
                        ? "Restarting..."
                        : "Restart"}
                    </button>
                  </Show>
                </div>
              </div>
            )}
          </Show>
        </div>

        {/* Agents summary card */}
        <div class="card">
          <h3>Agents</h3>
          <Show
            when={config()}
            fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}
          >
            {(cfg) => {
              const c = cfg() as Record<string, any>;
              const count = Object.keys(c.agents?.agents ?? {}).length;
              return (
                <p style={{ "font-size": "14px", color: "#8b949e" }}>
                  {count} agent{count !== 1 ? "s" : ""} configured
                </p>
              );
            }}
          </Show>
        </div>
      </div>

      <style>{`
        .card {
          background: #161b22;
          border: 1px solid #30363d;
          border-radius: 8px;
          padding: 20px;
        }
        .card h3 {
          font-size: 16px;
          font-weight: 600;
          margin-bottom: 12px;
          color: #f0f6fc;
        }
        .btn-primary {
          background: #238636;
          color: #fff;
          border: 1px solid #2ea043;
          border-radius: 6px;
          padding: 6px 16px;
          font-size: 13px;
          cursor: pointer;
          font-weight: 500;
        }
        .btn-primary:hover:not(:disabled) {
          background: #2ea043;
        }
        .btn-primary:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
        .btn-danger {
          background: #da3633;
          color: #fff;
          border: 1px solid #f85149;
          border-radius: 6px;
          padding: 6px 16px;
          font-size: 13px;
          cursor: pointer;
          font-weight: 500;
        }
        .btn-danger:hover:not(:disabled) {
          background: #f85149;
        }
        .btn-danger:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
        .btn-secondary {
          background: #21262d;
          color: #c9d1d9;
          border: 1px solid #30363d;
          border-radius: 6px;
          padding: 6px 16px;
          font-size: 13px;
          cursor: pointer;
          font-weight: 500;
        }
        .btn-secondary:hover:not(:disabled) {
          background: #30363d;
        }
        .btn-secondary:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
      `}</style>
    </div>
  );
}
