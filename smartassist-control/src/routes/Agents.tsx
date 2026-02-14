import { Show, For, createSignal } from "solid-js";
import { useConfig } from "../lib/useConfig";
import type { ThinkingLevel, ToolProfile, AgentConfig } from "../lib/types";
import {
  TextInput,
  EnumSelect,
  Section,
  SaveBar,
  Toast,
  styles,
} from "../components/FormComponents";

export default function Agents() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();
  const [editingAgent, setEditingAgent] = createSignal<string | null>(null);

  const agentEntries = () => {
    const c = config();
    if (!c) return [];
    return Object.entries(c.agents.agents);
  };

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>Agents</h1>

      <Show when={config()} fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Defaults">
              <TextInput
                label="Default Agent"
                value={cfg().agents.default ?? ""}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.agents.default = v || undefined;
                    return c;
                  })
                }
                help="ID of the default agent to use"
              />
              <TextInput
                label="Default Model"
                value={cfg().agents.defaults.model ?? ""}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.agents.defaults.model = v || undefined;
                    return c;
                  })
                }
                placeholder="provider/model-id"
                help="Format: provider/model-id (e.g. anthropic/claude-sonnet-4-5-20250929)"
              />
              <EnumSelect<ThinkingLevel>
                label="Default Thinking Level"
                value={cfg().agents.defaults.thinking_level}
                options={[
                  { value: "off", label: "Off" },
                  { value: "minimal", label: "Minimal" },
                  { value: "low", label: "Low" },
                  { value: "medium", label: "Medium" },
                  { value: "high", label: "High" },
                  { value: "xhigh", label: "XHigh" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.agents.defaults.thinking_level = v;
                    return c;
                  })
                }
              />
              <EnumSelect<ToolProfile>
                label="Default Tool Profile"
                value={cfg().agents.defaults.tools.profile}
                options={[
                  { value: "full", label: "Full (all tools)" },
                  { value: "coding", label: "Coding" },
                  { value: "messaging", label: "Messaging" },
                  { value: "minimal", label: "Minimal" },
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.agents.defaults.tools.profile = v;
                    return c;
                  })
                }
              />
            </Section>

            <Section title="Agents">
              <For each={agentEntries()}>
                {([id, agent]) => (
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
                        "margin-bottom": editingAgent() === id ? "16px" : "0",
                        cursor: "pointer",
                      }}
                      onClick={() =>
                        setEditingAgent(editingAgent() === id ? null : id)
                      }
                    >
                      <div>
                        <strong style={{ color: "#f0f6fc" }}>
                          {agent.name || id}
                        </strong>
                        <span
                          style={{
                            color: "#8b949e",
                            "font-size": "13px",
                            "margin-left": "8px",
                          }}
                        >
                          {agent.model ?? cfg().agents.defaults.model ?? "no model"}
                        </span>
                      </div>
                      <span style={{ color: "#8b949e", "font-size": "12px" }}>
                        {editingAgent() === id ? "Collapse" : "Edit"}
                      </span>
                    </div>

                    <Show when={editingAgent() === id}>
                      <TextInput
                        label="Name"
                        value={agent.name ?? ""}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.agents.agents[id].name = v || undefined;
                            return c;
                          })
                        }
                      />
                      <TextInput
                        label="Model"
                        value={agent.model ?? ""}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.agents.agents[id].model = v || undefined;
                            return c;
                          })
                        }
                        placeholder="provider/model-id"
                      />
                      <EnumSelect<ThinkingLevel>
                        label="Thinking Level"
                        value={agent.thinking_level}
                        options={[
                          { value: "off", label: "Off" },
                          { value: "minimal", label: "Minimal" },
                          { value: "low", label: "Low" },
                          { value: "medium", label: "Medium" },
                          { value: "high", label: "High" },
                          { value: "xhigh", label: "XHigh" },
                        ]}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.agents.agents[id].thinking_level = v;
                            return c;
                          })
                        }
                      />
                      <TextInput
                        label="System Prompt"
                        value={agent.system_prompt ?? ""}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.agents.agents[id].system_prompt = v || undefined;
                            return c;
                          })
                        }
                      />
                      <button
                        class={styles.btnDanger}
                        onClick={() =>
                          updateConfig((c) => {
                            delete c.agents.agents[id];
                            if (c.agents.default === id) {
                              c.agents.default = undefined;
                            }
                            return c;
                          })
                        }
                      >
                        Delete Agent
                      </button>
                    </Show>
                  </div>
                )}
              </For>

              <button
                class={styles.btnSecondary}
                onClick={() => {
                  const id = `agent-${Date.now()}`;
                  updateConfig((c) => {
                    c.agents.agents[id] = {
                      id,
                      name: "New Agent",
                      fallback_models: [],
                      thinking_level: "low",
                      tools: {
                        profile: "full",
                        allow: [],
                        deny: [],
                        also_allow: [],
                      },
                      subagents: { allow_agents: [] },
                    } as AgentConfig;
                    return c;
                  });
                  setEditingAgent(id);
                }}
              >
                + Add Agent
              </button>
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
