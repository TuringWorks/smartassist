import { A, useLocation } from "@solidjs/router";
import styles from "./Sidebar.module.css";

// ── Inline SVG icons (18×18) ─────────────────────────────────

const icons = {
  dashboard: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="3" y="14" width="7" height="7" rx="1" />
      <rect x="14" y="14" width="7" height="7" rx="1" />
    </svg>
  ),
  agents: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="8" r="5" />
      <path d="M20 21a8 8 0 1 0-16 0" />
    </svg>
  ),
  channels: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  ),
  gateway: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
      <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
      <line x1="6" y1="6" x2="6.01" y2="6" />
      <line x1="6" y1="18" x2="6.01" y2="18" />
    </svg>
  ),
  security: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
    </svg>
  ),
  sessions: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="10" />
      <polyline points="12 6 12 12 16 14" />
    </svg>
  ),
  routing: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="16 3 21 3 21 8" />
      <line x1="4" y1="20" x2="21" y2="3" />
      <polyline points="21 16 21 21 16 21" />
      <line x1="15" y1="15" x2="21" y2="21" />
      <line x1="4" y1="4" x2="9" y2="9" />
    </svg>
  ),
  memory: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <ellipse cx="12" cy="5" rx="9" ry="3" />
      <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3" />
      <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" />
    </svg>
  ),
  logging: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" y1="13" x2="8" y2="13" />
      <line x1="16" y1="17" x2="8" y2="17" />
    </svg>
  ),
  logs: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  ),
  liveSessions: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
    </svg>
  ),
  cron: (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
    </svg>
  ),
};

// ── Logo SVG ─────────────────────────────────────────────────

const LogoSvg = () => (
  <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
    <path d="M12 2L2 7l10 5 10-5-10-5z" />
    <path d="M2 17l10 5 10-5" />
    <path d="M2 12l10 5 10-5" />
  </svg>
);

// ── Nav Items ────────────────────────────────────────────────

interface NavItem {
  path: string;
  label: string;
  icon: any;
}

const configItems: NavItem[] = [
  { path: "/agents", label: "Agents", icon: icons.agents },
  { path: "/channels", label: "Channels", icon: icons.channels },
  { path: "/gateway", label: "Gateway", icon: icons.gateway },
  { path: "/security", label: "Security", icon: icons.security },
  { path: "/sessions-config", label: "Sessions", icon: icons.sessions },
  { path: "/routing", label: "Routing", icon: icons.routing },
  { path: "/memory", label: "Memory", icon: icons.memory },
  { path: "/logging", label: "Logging", icon: icons.logging },
];

const monitorItems: NavItem[] = [
  { path: "/logs", label: "Logs", icon: icons.logs },
  { path: "/sessions", label: "Live Sessions", icon: icons.liveSessions },
  { path: "/cron", label: "Cron Jobs", icon: icons.cron },
];

// ── Sidebar Component ────────────────────────────────────────

export default function Sidebar() {
  const location = useLocation();

  const isActive = (path: string) => location.pathname === path;

  return (
    <nav class={styles.sidebar}>
      {/* Logo */}
      <div class={styles.logo}>
        <div class={styles.logoIcon}>
          <LogoSvg />
        </div>
        <div class={styles.logoTextGroup}>
          <span class={styles.logoText}>SmartAssist</span>
          <span class={styles.logoSub}>Control Panel</span>
        </div>
      </div>

      {/* Navigation */}
      <div class={styles.navGroup}>
        <A
          href="/"
          class={`${styles.navItem} ${isActive("/") ? styles.active : ""}`}
        >
          {icons.dashboard}
          Dashboard
        </A>

        <div class={styles.sectionHeader}>Configuration</div>
        {configItems.map((item) => (
          <A
            href={item.path}
            class={`${styles.navItem} ${isActive(item.path) ? styles.active : ""}`}
          >
            {item.icon}
            {item.label}
          </A>
        ))}

        <div class={styles.sectionHeader}>Monitor</div>
        {monitorItems.map((item) => (
          <A
            href={item.path}
            class={`${styles.navItem} ${isActive(item.path) ? styles.active : ""}`}
          >
            {item.icon}
            {item.label}
          </A>
        ))}
      </div>

      {/* Footer */}
      <div class={styles.sidebarFooter}>
        <span class={styles.footerDot} />
        <span class={styles.footerText}>System Active</span>
      </div>
    </nav>
  );
}
