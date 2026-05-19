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
  
  // Add API Key State
  const [selectedProvider, setSelectedProvider] = createSignal("openai");
  const [keySuffix, setKeySuffix] = createSignal("");
  const [customKeyName, setCustomKeyName] = createSignal("");
  const [keyValue, setKeyValue] = createSignal("");
  
  const PREDEFINED_PROVIDERS = [
    { value: "openai", label: "OpenAI" },
    { value: "anthropic", label: "Anthropic" },
    { value: "google", label: "Google Gemini" },
    { value: "groq", label: "Groq" },
    { value: "together", label: "Together AI" },
    { value: "openrouter", label: "OpenRouter" },
    { value: "qwen", label: "Alibaba Qwen" },
    { value: "custom", label: "Custom Provider" },
  ];

  const generatedKeyName = () => {
    if (selectedProvider() === "custom") return customKeyName().trim();
    const suffix = keySuffix().trim();
    return suffix ? `${selectedProvider()}_api_key_${suffix}` : `${selectedProvider()}_api_key`;
  };

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
      setKeySuffix("");
      setKeyValue("");
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
            <Show when={secrets()?.length === 0}>
              <p class={styles.description}>No API keys configured yet.</p>
            </Show>
            <For each={secrets()}>
              {(secretName) => {
                const isEditing = () => editingSecret() === secretName;

                return (
                  <div class={styles.providerCard}>
                    <div class={isEditing() ? styles.providerHeaderEditing : styles.providerHeader}>
                      <div>
                        <strong class={styles.providerLabel}>{secretName}</strong>
                        <span class={styles.statusConfigured}>
                          [ENCRYPTED & SAVED]
                        </span>
                      </div>
                      <div class={styles.actionButtons}>
                        <button
                          class={formStyles.btnSecondary}
                          onClick={() => {
                            if (isEditing()) {
                              setEditingSecret(null);
                            } else {
                              setEditingSecret(secretName);
                              // Reset hidden input just in case
                              const input = document.getElementById(`overwrite-${secretName}`) as HTMLInputElement;
                              if (input) input.value = "";
                            }
                          }}
                        >
                          {isEditing() ? "Cancel" : "Overwrite"}
                        </button>
                        <button
                          class={formStyles.btnDanger}
                          onClick={() => handleDeleteSecret(secretName)}
                        >
                          Delete
                        </button>
                      </div>
                    </div>

                    {/* Configure / Overwrite inline form */}
                    <Show when={isEditing()}>
                      <SecretInput
                        label="New API Key Value"
                        value=""
                        placeholder="Enter new API Key"
                        onChange={(v) => {
                           const input = document.getElementById(`overwrite-${secretName}`) as HTMLInputElement;
                           if (input) input.value = v;
                        }}
                      />
                      <input type="hidden" id={`overwrite-${secretName}`} />
                      <button
                        class={`${formStyles.btnPrimary} ${styles.saveButton}`}
                        onClick={() => {
                          const val = (document.getElementById(`overwrite-${secretName}`) as HTMLInputElement)?.value;
                          if (val) handleSaveSecret(secretName, val);
                        }}
                      >
                        Save New Value
                      </button>
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>

        <div class={styles.customKeySection}>
          <h3 class={styles.customKeyHeader}>Add API Key</h3>
          <p class={styles.description}>Add a new API key for a provider. You can optionally add a suffix to identify multiple keys for the same provider.</p>
          
          <EnumSelect
            label="Provider"
            value={selectedProvider()}
            options={PREDEFINED_PROVIDERS}
            onChange={setSelectedProvider}
          />

          <Show when={selectedProvider() === "custom"}>
            <TextInput
              label="Custom Secret Name"
              value={customKeyName()}
              onChange={setCustomKeyName}
              placeholder="e.g. xai_api_key"
            />
          </Show>
          
          <Show when={selectedProvider() !== "custom"}>
            <TextInput
              label="Key Label/Suffix (Optional)"
              value={keySuffix()}
              onChange={setKeySuffix}
              placeholder="e.g. work, personal"
              help={`Generated Secret Name: ${generatedKeyName()}`}
            />
          </Show>

          <SecretInput
            label="API Key"
            value={keyValue()}
            onChange={setKeyValue}
            placeholder="Enter API Key"
          />
          <button
            class={`${formStyles.btnPrimary} ${styles.saveButton}`}
            disabled={!generatedKeyName() || !keyValue()}
            onClick={() => handleSaveSecret(generatedKeyName(), keyValue())}
          >
            Save API Key
          </button>
        </div>
      </Section>

      <Show when={toast()}>
        {(t) => <Toast message={t().message} type={t().type} visible={true} />}
      </Show>
    </div>
  );
}
