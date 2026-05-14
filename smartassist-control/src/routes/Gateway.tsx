import { Show } from "solid-js";
import { useConfig } from "../lib/useConfig";
import type { BindMode, ControlUiAuthMode, TailscaleMode } from "../lib/types";
import {
  EnumSelect,
  NumberInput,
  Toggle,
  SecretInput,
  Section,
  SaveBar,
  Toast,
  PageHeader,
} from "../components/FormComponents";

export default function GatewaySettings() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  return (
    <div style={{ "padding-bottom": "80px" }}>
      <PageHeader title="Gateway" subtitle="Network, authentication, and endpoint settings" />

      <Show when={config()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Network">
              <EnumSelect<BindMode>
                label="Bind Mode"
                value={cfg().gateway.bind}
                options={[
                  { value: "loopback", label: "Loopback (127.0.0.1)" },
                  { value: "lan", label: "LAN (0.0.0.0)" },
                  { value: "tailnet", label: "Tailnet" },
                  { value: "auto", label: "Auto" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.gateway.bind = v;
                    return c;
                  })
                }
                help="Loopback is safest. LAN/Tailnet expose the gateway on the network."
              />
              <NumberInput
                label="Port"
                value={cfg().gateway.port}
                min={1}
                max={65535}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.gateway.port = v;
                    return c;
                  })
                }
              />
            </Section>

            <Section title="Control UI">
              <Toggle
                label="Enable Control UI"
                value={cfg().gateway.control_ui.enabled}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.gateway.control_ui.enabled = v;
                    return c;
                  })
                }
              />
              <EnumSelect<ControlUiAuthMode>
                label="Auth Mode"
                value={cfg().gateway.control_ui.auth.mode}
                options={[
                  { value: "identity", label: "Identity" },
                  { value: "password", label: "Password" },
                  { value: "token", label: "Token" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.gateway.control_ui.auth.mode = v;
                    return c;
                  })
                }
              />
              <Show when={cfg().gateway.control_ui.auth.mode === "password"}>
                <SecretInput
                  label="Password"
                  value={cfg().gateway.control_ui.auth.password ?? ""}
                  onChange={(v) =>
                    updateConfig((c) => {
                      c.gateway.control_ui.auth.password = v || undefined;
                      return c;
                    })
                  }
                />
              </Show>
              <Show when={cfg().gateway.control_ui.auth.mode === "token"}>
                <SecretInput
                  label="Token"
                  value={cfg().gateway.control_ui.auth.token ?? ""}
                  onChange={(v) =>
                    updateConfig((c) => {
                      c.gateway.control_ui.auth.token = v || undefined;
                      return c;
                    })
                  }
                />
              </Show>
            </Section>

            <Section title="HTTP Endpoints" defaultOpen={false}>
              <Toggle
                label="Chat Completions API"
                value={cfg().gateway.http.endpoints.chat_completions.enabled}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.gateway.http.endpoints.chat_completions.enabled = v;
                    return c;
                  })
                }
              />
            </Section>

            <Section title="Tailscale" defaultOpen={false}>
              <EnumSelect<TailscaleMode>
                label="Mode"
                value={cfg().gateway.tailscale.mode}
                options={[
                  { value: "off", label: "Off" },
                  { value: "serve", label: "Serve" },
                  { value: "funnel", label: "Funnel" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.gateway.tailscale.mode = v;
                    return c;
                  })
                }
              />
            </Section>
          </>
        )}
      </Show>

      <SaveBar dirty={dirty()} saving={saving()} onSave={save} onDiscard={discard} />
      <Show when={toast()}>
        {(t) => <Toast message={t().message} type={t().type} visible={true} />}
      </Show>
    </div>
  );
}
