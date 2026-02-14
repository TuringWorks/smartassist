import { createSignal, onMount } from "solid-js";
import { getConfig, saveConfig } from "./tauri";
import type { Config } from "./types";

/**
 * Shared hook for config pages.
 * Loads the full config, tracks dirty state, and provides save/discard.
 */
export function useConfig() {
  const [config, setConfig] = createSignal<Config | null>(null);
  const [original, setOriginal] = createSignal<string>("");
  const [dirty, setDirty] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [toast, setToast] = createSignal<{
    message: string;
    type: "success" | "error";
  } | null>(null);

  const load = async () => {
    try {
      const raw = await getConfig();
      const cfg = raw as unknown as Config;
      setConfig(cfg);
      setOriginal(JSON.stringify(cfg));
      setDirty(false);
    } catch (e) {
      showToast(`Failed to load config: ${e}`, "error");
    }
  };

  onMount(() => {
    load();
  });

  const updateConfig = (updater: (c: Config) => Config) => {
    const c = config();
    if (!c) return;
    const updated = updater(structuredClone(c));
    setConfig(updated);
    setDirty(JSON.stringify(updated) !== original());
  };

  const save = async () => {
    const c = config();
    if (!c) return;
    setSaving(true);
    try {
      await saveConfig(c as unknown as Record<string, unknown>);
      setOriginal(JSON.stringify(c));
      setDirty(false);
      showToast("Configuration saved", "success");
    } catch (e) {
      showToast(`Save failed: ${e}`, "error");
    } finally {
      setSaving(false);
    }
  };

  const discard = () => {
    try {
      const orig = JSON.parse(original()) as Config;
      setConfig(orig);
      setDirty(false);
    } catch {
      // ignore
    }
  };

  const showToast = (message: string, type: "success" | "error") => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  return { config, dirty, saving, toast, updateConfig, save, discard };
}
