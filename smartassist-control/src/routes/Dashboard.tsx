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
import { PageHeader } from "../components/FormComponents";
import styles from "./Dashboard.module.css";

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

  // Compute channel count
  const channelCount = () => {
    const c = config() as Record<string, any>;
    if (!c?.channels) return 0;
    let count = 0;
    if (c.channels.telegram?.enabled) count++;
    if (c.channels.discord?.enabled) count++;
    if (c.channels.slack?.enabled) count++;
    if (c.channels.signal?.enabled) count++;
    if (c.channels.whatsapp?.enabled) count++;
    return count;
  };

  const agentCount = () => {
    const c = config() as Record<string, any>;
    if (!c?.agents?.agents) return 0;
    return Object.keys(c.agents.agents).length;
  };

  return (
    <div>
      <PageHeader
        title="Dashboard"
        subtitle="System overview and gateway management"
      />

      <div class={styles.dashGrid}>
        {/* SmartAssist Version Card */}
        <div class={styles.card}>
          <div class={styles.cardHeader}>
            <div class={styles.cardIconBlue}>
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 2L2 7l10 5 10-5-10-5z" />
                <path d="M2 17l10 5 10-5" />
                <path d="M2 12l10 5 10-5" />
              </svg>
            </div>
            <span class={styles.cardTitle}>SmartAssist</span>
          </div>
          <div class={styles.cardMeta}>
            <span>Version: {version() ?? "..."}</span>
          </div>
          <div class={styles.cardMetaMono}>
            Config: {configPath() ?? "..."}
          </div>
        </div>

        {/* Gateway Status Card */}
        <div class={styles.card}>
          <div class={styles.cardHeader}>
            <div class={styles.cardIconGreen}>
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
                <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
                <line x1="6" y1="6" x2="6.01" y2="6" />
                <line x1="6" y1="18" x2="6.01" y2="18" />
              </svg>
            </div>
            <span class={styles.cardTitle}>Gateway</span>
          </div>
          <Show
            when={gwStatus()}
            fallback={<div class={styles.cardMeta}>Loading...</div>}
          >
            {(gw) => (
              <div>
                <div class={styles.statusRow}>
                  <span
                    class={gw().running ? styles.statusDotRunning : styles.statusDotStopped}
                  />
                  <span
                    class={gw().running ? styles.statusRunning : styles.statusStopped}
                  >
                    {gw().running ? "Running" : "Stopped"}
                  </span>
                </div>
                <div class={styles.cardMeta}>
                  <span>Port: {gw().port}</span>
                  <Show when={gw().running && gw().pid}>
                    <span>PID: {gw().pid}</span>
                  </Show>
                  <Show when={gw().running && gw().uptime_secs != null}>
                    <span>Uptime: {formatUptime(gw().uptime_secs!)}</span>
                  </Show>
                </div>

                <div class={styles.btnGroup}>
                  <Show when={!gw().running}>
                    <button
                      class={styles.btnGreen}
                      disabled={gwAction() !== null}
                      onClick={handleStart}
                    >
                      {gwAction() === "starting" ? "Starting..." : "Start"}
                    </button>
                  </Show>
                  <Show when={gw().running}>
                    <button
                      class={styles.btnRed}
                      disabled={gwAction() !== null}
                      onClick={handleStop}
                    >
                      {gwAction() === "stopping" ? "Stopping..." : "Stop"}
                    </button>
                    <button
                      class={styles.btnGhost}
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

        {/* Agents Count Card */}
        <div class={styles.card}>
          <div class={styles.cardHeader}>
            <div class={styles.cardIconPurple}>
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="8" r="5" />
                <path d="M20 21a8 8 0 1 0-16 0" />
              </svg>
            </div>
            <span class={styles.cardTitle}>Agents</span>
          </div>
          <Show
            when={config()}
            fallback={<div class={styles.cardMeta}>Loading...</div>}
          >
            <div class={styles.statValue}>{agentCount()}</div>
            <div class={styles.statLabel}>
              agent{agentCount() !== 1 ? "s" : ""} configured
            </div>
          </Show>
        </div>

        {/* Channels Count Card */}
        <div class={styles.card}>
          <div class={styles.cardHeader}>
            <div class={styles.cardIconCyan}>
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
              </svg>
            </div>
            <span class={styles.cardTitle}>Channels</span>
          </div>
          <Show
            when={config()}
            fallback={<div class={styles.cardMeta}>Loading...</div>}
          >
            <div class={styles.statValue}>{channelCount()}</div>
            <div class={styles.statLabel}>
              channel{channelCount() !== 1 ? "s" : ""} enabled
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}
