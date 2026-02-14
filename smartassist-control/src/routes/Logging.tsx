import { Show } from "solid-js";
import { useConfig } from "../lib/useConfig";
import type { LogLevel } from "../lib/types";
import {
  EnumSelect,
  TextInput,
  Toggle,
  Section,
  SaveBar,
  Toast,
} from "../components/FormComponents";

export default function Logging() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>Logging</h1>

      <Show when={config()} fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Log Settings">
              <EnumSelect<LogLevel>
                label="Log Level"
                value={cfg().logging.level}
                options={[
                  { value: "trace", label: "Trace" },
                  { value: "debug", label: "Debug" },
                  { value: "info", label: "Info" },
                  { value: "warn", label: "Warn" },
                  { value: "error", label: "Error" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.logging.level = v;
                    return c;
                  })
                }
              />
              <TextInput
                label="Log File Path"
                value={cfg().logging.file ?? ""}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.logging.file = v || undefined;
                    return c;
                  })
                }
                placeholder="~/.smartassist/smartassist.log"
                help="Leave empty to log to stdout only"
              />
            </Section>

            <Section title="Diagnostics" defaultOpen={false}>
              <Toggle
                label="Enable Diagnostics"
                value={cfg().logging.diagnostics.enabled}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.logging.diagnostics.enabled = v;
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
