import { Show } from "solid-js";
import { useConfig } from "../lib/useConfig";
import type { SessionScope, DmScope, SessionResetMode } from "../lib/types";
import {
  EnumSelect,
  NumberInput,
  Section,
  SaveBar,
  Toast,
} from "../components/FormComponents";

export default function SessionConfig() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Sessions
      </h1>

      <Show when={config()} fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Session Scope">
              <EnumSelect<SessionScope>
                label="Scope"
                value={cfg().session.scope}
                options={[
                  { value: "per_sender", label: "Per Sender" },
                  { value: "global", label: "Global" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.session.scope = v;
                    return c;
                  })
                }
                help="Per Sender creates separate sessions for each user. Global shares one session."
              />
              <EnumSelect<DmScope>
                label="DM Scope"
                value={cfg().session.dm_scope}
                options={[
                  { value: "main", label: "Main (shared)" },
                  { value: "per_peer", label: "Per Peer" },
                  { value: "per_channel_peer", label: "Per Channel + Peer" },
                  {
                    value: "per_account_channel_peer",
                    label: "Per Account + Channel + Peer",
                  },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.session.dm_scope = v;
                    return c;
                  })
                }
                help="Controls how DM sessions are isolated"
              />
            </Section>

            <Section title="Session Reset">
              <EnumSelect<SessionResetMode>
                label="Reset Mode"
                value={cfg().session.reset.mode}
                options={[
                  { value: "daily", label: "Daily" },
                  { value: "idle", label: "After Idle" },
                  { value: "never", label: "Never" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.session.reset.mode = v;
                    return c;
                  })
                }
              />
              <Show when={cfg().session.reset.mode === "daily"}>
                <NumberInput
                  label="Reset Hour (0-23)"
                  value={cfg().session.reset.at_hour}
                  min={0}
                  max={23}
                  onChange={(v) =>
                    updateConfig((c) => {
                      c.session.reset.at_hour = v;
                      return c;
                    })
                  }
                  help="Hour of day (UTC) to reset sessions"
                />
              </Show>
              <Show when={cfg().session.reset.mode === "idle"}>
                <NumberInput
                  label="Idle Minutes"
                  value={cfg().session.reset.idle_minutes}
                  min={1}
                  max={1440}
                  onChange={(v) =>
                    updateConfig((c) => {
                      c.session.reset.idle_minutes = v;
                      return c;
                    })
                  }
                  help="Minutes of inactivity before session reset"
                />
              </Show>
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
