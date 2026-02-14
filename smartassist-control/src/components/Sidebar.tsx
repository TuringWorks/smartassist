import { A, useLocation } from "@solidjs/router";
import styles from "./Sidebar.module.css";

interface NavItem {
  path: string;
  label: string;
  section: string;
}

const configItems: NavItem[] = [
  { path: "/agents", label: "Agents", section: "config" },
  { path: "/channels", label: "Channels", section: "config" },
  { path: "/gateway", label: "Gateway", section: "config" },
  { path: "/security", label: "Security", section: "config" },
  { path: "/sessions-config", label: "Sessions", section: "config" },
  { path: "/routing", label: "Routing", section: "config" },
  { path: "/memory", label: "Memory", section: "config" },
  { path: "/logging", label: "Logging", section: "config" },
];

const monitorItems: NavItem[] = [
  { path: "/logs", label: "Logs", section: "monitor" },
  { path: "/sessions", label: "Live Sessions", section: "monitor" },
  { path: "/cron", label: "Cron Jobs", section: "monitor" },
];

export default function Sidebar() {
  const location = useLocation();

  const isActive = (path: string) => location.pathname === path;

  return (
    <nav class={styles.sidebar}>
      <div class={styles.logo}>
        <span class={styles.logoText}>SmartAssist</span>
        <span class={styles.logoSub}>Control Panel</span>
      </div>

      <A
        href="/"
        class={`${styles.navItem} ${isActive("/") ? styles.active : ""}`}
      >
        Dashboard
      </A>

      <div class={styles.sectionHeader}>Configuration</div>
      {configItems.map((item) => (
        <A
          href={item.path}
          class={`${styles.navItem} ${isActive(item.path) ? styles.active : ""}`}
        >
          {item.label}
        </A>
      ))}

      <div class={styles.sectionHeader}>Monitor</div>
      {monitorItems.map((item) => (
        <A
          href={item.path}
          class={`${styles.navItem} ${isActive(item.path) ? styles.active : ""}`}
        >
          {item.label}
        </A>
      ))}
    </nav>
  );
}
