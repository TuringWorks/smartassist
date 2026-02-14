import { Show } from "solid-js";
import { useConfig } from "../lib/useConfig";
import type {
  ExecMode,
  AskMode,
  AskFallback,
  DmPolicy,
  SandboxProfile,
} from "../lib/types";
import {
  EnumSelect,
  NumberInput,
  Toggle,
  TextInput,
  Section,
  SaveBar,
  Toast,
} from "../components/FormComponents";

export default function Security() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Security
      </h1>

      <Show when={config()} fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Execution Policy">
              <EnumSelect<ExecMode>
                label="Execution Mode"
                value={cfg().security.exec.mode}
                options={[
                  { value: "deny", label: "Deny (block all)" },
                  { value: "allowlist", label: "Allowlist" },
                  { value: "full", label: "Full (allow all)" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.mode = v;
                    return c;
                  })
                }
                help="Controls whether agents can execute commands"
              />
              <EnumSelect<AskMode>
                label="Approval Mode"
                value={cfg().security.exec.ask}
                options={[
                  { value: "off", label: "Off (never ask)" },
                  { value: "on_miss", label: "On Miss (ask when not in allowlist)" },
                  { value: "always", label: "Always" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.ask = v;
                    return c;
                  })
                }
              />
              <EnumSelect<AskFallback>
                label="Approval Fallback"
                value={cfg().security.exec.ask_fallback}
                options={[
                  { value: "deny", label: "Deny" },
                  { value: "allow", label: "Allow" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.ask_fallback = v;
                    return c;
                  })
                }
                help="What to do when approval request times out"
              />
              <NumberInput
                label="Approval Timeout (seconds)"
                value={cfg().security.exec.approval_timeout_secs}
                min={10}
                max={600}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.exec.approval_timeout_secs = v;
                    return c;
                  })
                }
              />
            </Section>

            <Section title="DM Policy">
              <EnumSelect<DmPolicy>
                label="Direct Message Policy"
                value={cfg().security.dm_policy}
                options={[
                  { value: "open", label: "Open (allow all)" },
                  { value: "pairing", label: "Pairing (require pairing)" },
                  { value: "allowlist", label: "Allowlist" },
                  { value: "blocked", label: "Blocked" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.dm_policy = v;
                    return c;
                  })
                }
              />
            </Section>

            <Section title="Audit Logging" defaultOpen={false}>
              <Toggle
                label="Enable Audit Logging"
                value={cfg().security.audit.enabled}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.audit.enabled = v;
                    return c;
                  })
                }
              />
              <TextInput
                label="Log Path"
                value={cfg().security.audit.log_path ?? ""}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.audit.log_path = v || undefined;
                    return c;
                  })
                }
                placeholder="~/.smartassist/audit/audit.log"
              />
              <Show when={cfg().security.audit.enabled}>
                <div style={{ "margin-top": "12px" }}>
                  <strong
                    style={{
                      "font-size": "13px",
                      color: "#c9d1d9",
                      display: "block",
                      "margin-bottom": "8px",
                    }}
                  >
                    Event Filters
                  </strong>
                  <Toggle
                    label="Exec events"
                    value={cfg().security.audit.events.exec}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.exec = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Auth events"
                    value={cfg().security.audit.events.auth}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.auth = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Channel events"
                    value={cfg().security.audit.events.channel}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.channel = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Security events"
                    value={cfg().security.audit.events.security}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.security = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Config events"
                    value={cfg().security.audit.events.config}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.config = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Session events"
                    value={cfg().security.audit.events.session}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.session = v;
                        return c;
                      })
                    }
                  />
                  <Toggle
                    label="Agent events"
                    value={cfg().security.audit.events.agent}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.security.audit.events.agent = v;
                        return c;
                      })
                    }
                  />
                </div>
              </Show>
            </Section>

            <Section title="Sandbox Defaults" defaultOpen={false}>
              <EnumSelect<SandboxProfile>
                label="Default Profile"
                value={cfg().security.sandbox.default_profile}
                options={[
                  { value: "strict", label: "Strict" },
                  { value: "standard", label: "Standard" },
                  { value: "trusted", label: "Trusted" },
                  { value: "none", label: "None" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.sandbox.default_profile = v;
                    return c;
                  })
                }
              />
              <NumberInput
                label="Max CPU Seconds"
                value={cfg().security.sandbox.default_limits.max_cpu_seconds}
                min={1}
                max={300}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.sandbox.default_limits.max_cpu_seconds = v;
                    return c;
                  })
                }
              />
              <NumberInput
                label="Max Processes"
                value={cfg().security.sandbox.default_limits.max_processes}
                min={1}
                max={100}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.security.sandbox.default_limits.max_processes = v;
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
