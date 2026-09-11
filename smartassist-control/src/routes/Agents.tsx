import { Show, Index, createSignal, For, createResource } from "solid-js";
import { useConfig } from "../lib/useConfig";
import { listSecrets } from "../lib/tauri";
import type { ThinkingLevel, ToolProfile, AgentConfig } from "../lib/types";
import {
  TextInput,
  EnumSelect,
  Section,
  SaveBar,
  Toast,
  PageHeader,
  styles,
} from "../components/FormComponents";

const POPULAR_MODELS = [
  { value: "anthropic/claude-opus-5", label: "Claude Opus 5" },
  { value: "anthropic/claude-sonnet-5", label: "Claude Sonnet 5" },
  { value: "anthropic/claude-haiku-4-5", label: "Claude Haiku 4.5" },
  { value: "openai/gpt-5.6-sol", label: "GPT-5.6 Sol" },
  { value: "openai/gpt-5.6-terra", label: "GPT-5.6 Terra" },
  { value: "openai/gpt-5.6-luna", label: "GPT-5.6 Luna" },
  { value: "google/gemini-3.6-flash", label: "Gemini 3.6 Flash" },
  { value: "google/gemini-3.1-pro-preview", label: "Gemini 3.1 Pro" },
  { value: "deepseek/deepseek-v4-pro", label: "DeepSeek V4 Pro" },
  { value: "deepseek/deepseek-v4-flash", label: "DeepSeek V4 Flash" },
  { value: "zhipu/glm-5.2", label: "GLM-5.2" },
  { value: "poolside/laguna-s-2.1", label: "Poolside Laguna S 2.1" },
  { value: "poolside/laguna-m-1", label: "Poolside Laguna M 1" },
  { value: "ollama/llama4", label: "Ollama Llama 4 (local)" },
  { value: "ollama/qwen3-coder", label: "Ollama Qwen3 Coder (local)" },
];

function ModelSelect(props: { label: string; value: string; onChange: (v: string) => void; help?: string }) {
  // If the current value is not in our popular list and isn't empty, we assume it's custom.
  const isKnown = () => POPULAR_MODELS.some(m => m.value === props.value) || props.value === "";
  const [isCustom, setIsCustom] = createSignal(!isKnown());

  return (
    <div class={styles.field}>
      <label class={styles.label}>{props.label}</label>
      <Show when={!isCustom()} fallback={
        <div style={{ display: "flex", gap: "8px" }}>
          <input
            class={styles.input}
            type="text"
            value={props.value}
            placeholder="provider/model-id"
            onInput={(e) => props.onChange(e.currentTarget.value)}
          />
          <button 
            class={styles.btnSecondary} 
            onClick={() => { setIsCustom(false); props.onChange(POPULAR_MODELS[0].value); }}
            style={{ "white-space": "nowrap" }}
          >
            Select List
          </button>
        </div>
      }>
        <div style={{ display: "flex", gap: "8px" }}>
          <select
            class={styles.select}
            aria-label={props.label}
            value={props.value}
            onChange={(e) => {
              if (e.currentTarget.value === "custom") {
                setIsCustom(true);
                props.onChange("");
              } else {
                props.onChange(e.currentTarget.value);
              }
            }}
          >
            <option value="">Select a model...</option>
            <For each={POPULAR_MODELS}>
              {(opt) => <option value={opt.value}>{opt.label} ({opt.value})</option>}
            </For>
            <option value="custom">Custom model...</option>
          </select>
        </div>
      </Show>
      <Show when={props.help}>
        <div class={styles.help}>{props.help}</div>
      </Show>
    </div>
  );
}

export default function Agents() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();
  const [editingAgent, setEditingAgent] = createSignal<string | null>(null);
  const [secrets] = createResource(listSecrets);

  const agentEntries = () => {
    const c = config();
    if (!c) return [];
    return Object.entries(c.agents.agents);
  };

  return (
    <div style={{ "padding-bottom": "80px" }}>
      <PageHeader title="Agents" subtitle="Manage AI agent configurations and defaults" />

      <Show when={config()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Defaults">
              <EnumSelect<string>
                label="Default Agent"
                value={cfg().agents.default ?? ""}
                options={[
                  { value: "", label: "None" },
                  ...Object.values(cfg().agents.agents).map(a => ({
                    value: a.id,
                    label: a.name ? `${a.name} (${a.id})` : a.id
                  }))
                ]}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.agents.default = v || undefined;
                    return c;
                  })
                }
                help="Select the default agent to use"
              />
              <ModelSelect
                label="Default Model"
                value={cfg().agents.defaults.model ?? ""}
                onChange={(v) =>
                  updateConfig((c) => {
                    c.agents.defaults.model = v || undefined;
                    return c;
                  })
                }
                help="Format: provider/model-id (e.g. anthropic/claude-sonnet-5)"
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
              <Index each={agentEntries()}>
                {(entry) => {
                  const [id, agent] = entry();
                  return (
                    <div
                      style={{
                        background: "var(--bg-input)",
                        border: "1px solid var(--border-primary)",
                        "border-radius": "var(--radius-sm)",
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
                          <strong style={{ color: "var(--text-heading)" }}>
                            {agent.name || id}
                          </strong>
                          <span
                            style={{
                              color: "var(--text-tertiary)",
                              "font-size": "13px",
                              "margin-left": "8px",
                            }}
                          >
                            {agent.model ?? cfg().agents.defaults.model ?? "no model"}
                          </span>
                        </div>
                        <span style={{ color: "var(--text-tertiary)", "font-size": "12px", "font-weight": "500" }}>
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
                        <ModelSelect
                          label="Model"
                          value={agent.model ?? ""}
                          onChange={(v) =>
                            updateConfig((c) => {
                              c.agents.agents[id].model = v || undefined;
                              return c;
                            })
                          }
                        />
                        <EnumSelect
                          label="API Key"
                          value={agent.api_key_name ?? ""}
                          options={[
                            { value: "", label: "Default Key for Provider" },
                            ...(secrets() || []).map((s) => ({ value: s, label: s }))
                          ]}
                          onChange={(v) =>
                            updateConfig((c) => {
                              c.agents.agents[id].api_key_name = v || undefined;
                              return c;
                            })
                          }
                          help="Select a specific API key to use, or leave as Default."
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
                  );
                }}
              </Index>

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
