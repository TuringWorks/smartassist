import { createSignal, createResource, Show, For } from "solid-js";
import {
  Section,
  EnumSelect,
  SecretInput,
  TextInput,
  PageHeader,
  Toast,
  styles as formStyles,
} from "../components/FormComponents";
import { theme, updateTheme } from "../App";
import { listSecrets, setSecret, deleteSecret } from "../lib/tauri";
import styles from "./Settings.module.css";

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
  
  // Custom API Key State
  const [customKeyName, setCustomKeyName] = createSignal("");
  const [customKeyValue, setCustomKeyValue] = createSignal("");

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
      setCustomKeyName("");
      setCustomKeyValue("");
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
    <div class={styles.container}>
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
        <p class={styles.description}>
          API keys are encrypted and stored securely by the OS keychain. Once set, they cannot be viewed, only overwritten or deleted.
        </p>

        <Show when={secrets.loading}>
          <p class={styles.loading}>Loading secrets...</p>
        </Show>
        <Show when={secrets.error}>
          <p class={styles.error}>Error loading secrets.</p>
        </Show>
        
        <Show when={secrets()}>
          <div class={styles.providerList}>
            <For each={(() => {
              const base = [
                { id: "openai_api_key", label: "OpenAI API Key", prefix: "sk-" },
                { id: "anthropic_api_key", label: "Anthropic API Key", prefix: "sk-ant-" },
                { id: "google_api_key", label: "Google Gemini API Key", prefix: "AIza" },
                { id: "groq_api_key", label: "Groq API Key", prefix: "gsk_" },
                { id: "together_api_key", label: "Together AI API Key", prefix: "" },
              ];
              const existingIds = new Set(base.map(p => p.id));
              const custom = (secrets() || [])
                .filter(s => !existingIds.has(s))
                .map(s => ({ id: s, label: s, prefix: "" }));
              return [...base, ...custom];
            })()}>
              {(provider) => {
                const isSet = () => secrets()?.includes(provider.id) ?? false;
                const isEditing = () => editingSecret() === provider.id;

                return (
                  <div class={styles.providerCard}>
                    <div class={isEditing() ? styles.providerHeaderEditing : styles.providerHeader}>
                      <div>
                        <strong class={styles.providerLabel}>{provider.label}</strong>
                        <span class={isSet() ? styles.statusConfigured : styles.statusUnconfigured}>
                          {isSet() ? "[ENCRYPTED & SAVED]" : "Not Configured"}
                        </span>
                      </div>
                      <div class={styles.actionButtons}>
                        <button
                          class={formStyles.btnSecondary}
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
                            class={formStyles.btnDanger}
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
                        class={`${formStyles.btnPrimary} ${styles.saveButton}`}
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

        <div class={styles.customKeySection}>
          <h3 class={styles.customKeyHeader}>Add Custom API Key</h3>
          <p class={styles.description}>Add credentials for other providers (e.g. `deepseek_api_key`).</p>
          <TextInput
            label="Provider Secret Name"
            value={customKeyName()}
            onChange={setCustomKeyName}
            placeholder="e.g. xai_api_key"
          />
          <SecretInput
            label="API Key"
            value={customKeyValue()}
            onChange={setCustomKeyValue}
            placeholder="Enter API Key"
          />
          <button
            class={`${formStyles.btnPrimary} ${styles.saveButton}`}
            disabled={!customKeyName() || !customKeyValue()}
            onClick={() => handleSaveSecret(customKeyName(), customKeyValue())}
          >
            Save Custom API Key
          </button>
        </div>
      </Section>

      <Show when={toast()}>
        {(t) => <Toast message={t().message} type={t().type} visible={true} />}
      </Show>
    </div>
  );
}
