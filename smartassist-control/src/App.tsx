import { Router, Route } from "@solidjs/router";
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
import "./App.module.css";

function Layout(props: { children?: any }) {
  return (
    <div class="app-layout">
      <Sidebar />
      <main class="app-content">{props.children}</main>
    </div>
  );
}

export default function App() {
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
    </Router>
  );
}
