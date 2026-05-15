import { createSignal, createResource, Show, For } from "solid-js";
import {
  Section,
  EnumSelect,
  SecretInput,
  PageHeader,
  Toast,
  styles,
} from "../components/FormComponents";
import { theme, updateTheme } from "../App";
import { listSecrets, setSecret, deleteSecret } from "../lib/tauri";

export default function Settings() {
  const [toast, setToast] = createSignal<{
    message: string;
    type: "success" | "error";
  } | null>(null);

  // ── Theme State ────────────────────────────────────────────────
  const handleThemeChange = (newTheme: string) => {
    updateTheme(newTheme);
  };

  // ── Secrets State ──────────────────────────────────────────────
  const [secrets, { refetch }] = createResource(listSecrets);
  
  const [editingSecret, setEditingSecret] = createSignal<string | null>(null);

  const showToast = (message: string, type: "success" | "error" = "success") => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const handleSaveSecret = async (name: string, value: string) => {
    if (!name || !value) {
      showToast("Name and value are required", "error");
      return;
    }
    
    try {
      await setSecret(name, value);
      showToast(`Secret '${name}' saved successfully`);
      setEditingSecret(null);
      refetch();
    } catch (e: any) {
      showToast(`Failed to save secret: ${e.toString()}`, "error");
    }
  };

  const handleDeleteSecret = async (name: string) => {
    if (!confirm(`Are you sure you want to delete the secret '${name}'?`)) return;
    try {
      await deleteSecret(name);
      showToast(`Secret '${name}' deleted`);
      refetch();
    } catch (e: any) {
      showToast(`Failed to delete secret: ${e.toString()}`, "error");
    }
  };

  return (
    <div style={{ "padding-bottom": "80px" }}>
      <PageHeader title="Settings" subtitle="Manage application appearance and secure provider credentials" />

      {/* ── Appearance Section ── */}
      <Section title="Appearance" defaultOpen={true}>
        <EnumSelect
          label="UI Theme"
          value={theme()}
          options={[
            { value: "dark", label: "Dark Mode" },
            { value: "light", label: "Light Mode" },
          ]}
          onChange={handleThemeChange}
          help="Choose the visual theme for the SmartAssist Control Panel."
        />
      </Section>

      {/* ── Provider API Keys Section ── */}
      <Section title="Provider API Keys" defaultOpen={true}>
        <p style={{ color: "var(--text-secondary)", "font-size": "14px", "margin-bottom": "16px" }}>
          API keys are encrypted and stored securely by the OS keychain. Once set, they cannot be viewed, only overwritten or deleted.
        </p>

        <Show when={secrets.loading}>
          <p style={{ color: "var(--text-tertiary)" }}>Loading secrets...</p>
        </Show>
        <Show when={secrets.error}>
          <p style={{ color: "var(--accent-red)" }}>Error loading secrets.</p>
        </Show>
        
        <Show when={secrets()}>
          <div style={{ "margin-bottom": "24px" }}>
            <For each={[
              { id: "openai_api_key", label: "OpenAI API Key", prefix: "sk-" },
              { id: "anthropic_api_key", label: "Anthropic API Key", prefix: "sk-ant-" },
              { id: "google_api_key", label: "Google Gemini API Key", prefix: "AIza" },
              { id: "groq_api_key", label: "Groq API Key", prefix: "gsk_" },
              { id: "together_api_key", label: "Together AI API Key", prefix: "" },
            ]}>
              {(provider) => {
                const isSet = () => secrets()?.includes(provider.id) ?? false;
                const isEditing = () => editingSecret() === provider.id;

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
                        "margin-bottom": isEditing() ? "16px" : "0",
                      }}
                    >
                      <div>
                        <strong style={{ color: "var(--text-heading)", "display": "block" }}>{provider.label}</strong>
                        <span style={{ 
                          color: isSet() ? "var(--accent-green)" : "var(--text-tertiary)", 
                          "font-size": "13px" 
                        }}>
                          {isSet() ? "[ENCRYPTED & SAVED]" : "Not Configured"}
                        </span>
                      </div>
                      <div>
                        <button
                          class={styles.btnSecondary}
                          style={{ "margin-right": "8px" }}
                          onClick={() => {
                            if (isEditing()) {
                              setEditingSecret(null);
                            } else {
                              setEditingSecret(provider.id);
                              // Reset hidden input just in case
                              const input = document.getElementById(`overwrite-${provider.id}`) as HTMLInputElement;
                              if (input) input.value = "";
                            }
                          }}
                        >
                          {isEditing() ? "Cancel" : (isSet() ? "Overwrite" : "Configure")}
                        </button>
                        <Show when={isSet()}>
                          <button
                            class={styles.btnDanger}
                            onClick={() => handleDeleteSecret(provider.id)}
                          >
                            Delete
                          </button>
                        </Show>
                      </div>
                    </div>

                    {/* Configure / Overwrite inline form */}
                    <Show when={isEditing()}>
                      <SecretInput
                        label="API Key Value"
                        value=""
                        placeholder={provider.prefix ? `e.g. ${provider.prefix}...` : "Enter API Key"}
                        onChange={(v) => {
                           const input = document.getElementById(`overwrite-${provider.id}`) as HTMLInputElement;
                           if (input) input.value = v;
                        }}
                      />
                      <input type="hidden" id={`overwrite-${provider.id}`} />
                      <button
                        class={styles.btnPrimary}
                        style={{ "margin-top": "8px" }}
                        onClick={() => {
                          const val = (document.getElementById(`overwrite-${provider.id}`) as HTMLInputElement)?.value;
                          if (val) handleSaveSecret(provider.id, val);
                        }}
                      >
                        {isSet() ? "Save New Value" : "Save API Key"}
                      </button>
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </Section>

      <Show when={toast()}>
        {(t) => <Toast message={t().message} type={t().type} visible={true} />}
      </Show>
    </div>
  );
}
