import { Show, For } from "solid-js";
import { useConfig } from "../lib/useConfig";
import {
  TextInput,
  Section,
  SaveBar,
  Toast,
  styles,
} from "../components/FormComponents";

export default function Routing() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>Routing</h1>

      <Show when={config()} fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}>
        {(cfg) => (
          <Section title="Route Bindings">
            <p
              style={{
                color: "#8b949e",
                "font-size": "13px",
                "margin-bottom": "16px",
              }}
            >
              Route incoming messages to specific agents based on channel,
              account, peer, or guild.
            </p>

            <For each={cfg().routing.bindings}>
              {(binding, index) => (
                <div
                  style={{
                    background: "#0d1117",
                    border: "1px solid #30363d",
                    "border-radius": "6px",
                    padding: "16px",
                    "margin-bottom": "12px",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      "justify-content": "space-between",
                      "align-items": "center",
                      "margin-bottom": "12px",
                    }}
                  >
                    <strong style={{ color: "#f0f6fc" }}>
                      Route #{index() + 1}
                    </strong>
                    <button
                      class={styles.btnDanger}
                      style={{ padding: "4px 12px", "font-size": "12px" }}
                      onClick={() =>
                        updateConfig((c) => {
                          c.routing.bindings.splice(index(), 1);
                          return c;
                        })
                      }
                    >
                      Remove
                    </button>
                  </div>
                  <TextInput
                    label="Agent ID"
                    value={binding.agent_id}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.routing.bindings[index()].agent_id = v;
                        return c;
                      })
                    }
                    placeholder="default"
                  />
                  <TextInput
                    label="Match Channel"
                    value={binding.match_channel ?? ""}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.routing.bindings[index()].match_channel =
                          v || undefined;
                        return c;
                      })
                    }
                    placeholder="telegram, discord, slack..."
                  />
                  <TextInput
                    label="Match Account"
                    value={binding.match_account ?? ""}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.routing.bindings[index()].match_account =
                          v || undefined;
                        return c;
                      })
                    }
                  />
                  <TextInput
                    label="Match Peer"
                    value={binding.match_peer ?? ""}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.routing.bindings[index()].match_peer =
                          v || undefined;
                        return c;
                      })
                    }
                  />
                  <TextInput
                    label="Match Guild/Team"
                    value={binding.match_guild ?? ""}
                    onChange={(v) =>
                      updateConfig((c) => {
                        c.routing.bindings[index()].match_guild =
                          v || undefined;
                        return c;
                      })
                    }
                  />
                </div>
              )}
            </For>

            <button
              class={styles.btnSecondary}
              onClick={() =>
                updateConfig((c) => {
                  c.routing.bindings.push({ agent_id: "" });
                  return c;
                })
              }
            >
              + Add Route
            </button>
          </Section>
        )}
      </Show>

      <SaveBar dirty={dirty()} saving={saving()} onSave={save} onDiscard={discard} />
      <Show when={toast()}>
        {(t) => <Toast message={t().message} type={t().type} visible={true} />}
      </Show>
    </div>
  );
}
