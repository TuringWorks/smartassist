import { Router, Route } from "@solidjs/router";
import { onMount, createEffect, createSignal } from "solid-js";
import Sidebar from "./components/Sidebar";
import Dashboard from "./routes/Dashboard";
import Agents from "./routes/Agents";
import Channels from "./routes/Channels";
import GatewaySettings from "./routes/Gateway";
import Security from "./routes/Security";
import SessionConfig from "./routes/SessionConfig";
import Routing from "./routes/Routing";
import Memory from "./routes/Memory";
import Logging from "./routes/Logging";
import Logs from "./routes/Logs";
import Sessions from "./routes/Sessions";
import Cron from "./routes/Cron";
import Settings from "./routes/Settings";
import "./App.module.css";

// Global theme signal
export const [theme, setTheme] = createSignal(localStorage.getItem("theme") || "dark");

export function updateTheme(newTheme: string) {
  setTheme(newTheme);
  localStorage.setItem("theme", newTheme);
  if (newTheme === "light") {
    document.documentElement.setAttribute("data-theme", "light");
  } else {
    document.documentElement.removeAttribute("data-theme");
  }
}

function Layout(props: { children?: any }) {
  return (
    <div class="app-layout">
      <Sidebar />
      <main class="app-content">{props.children}</main>
    </div>
  );
}

export default function App() {
  onMount(() => {
    // Initialize theme on load
    if (theme() === "light") {
      document.documentElement.setAttribute("data-theme", "light");
    }
  });

  return (
    <Router root={Layout}>
      <Route path="/" component={Dashboard} />
      <Route path="/agents" component={Agents} />
      <Route path="/channels" component={Channels} />
      <Route path="/gateway" component={GatewaySettings} />
      <Route path="/security" component={Security} />
      <Route path="/sessions-config" component={SessionConfig} />
      <Route path="/routing" component={Routing} />
      <Route path="/memory" component={Memory} />
      <Route path="/logging" component={Logging} />
      <Route path="/logs" component={Logs} />
      <Route path="/sessions" component={Sessions} />
      <Route path="/cron" component={Cron} />
      <Route path="/settings" component={Settings} />
    </Router>
  );
}
