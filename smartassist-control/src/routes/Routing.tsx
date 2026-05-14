import { Show, Index } from "solid-js";
import { useConfig } from "../lib/useConfig";
import {
  TextInput,
  Section,
  SaveBar,
  Toast,
  PageHeader,
  styles,
} from "../components/FormComponents";

export default function Routing() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  return (
    <div style={{ "padding-bottom": "80px" }}>
      <PageHeader title="Routing" subtitle="Route messages to agents by channel, account, or peer" />

      <Show when={config()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Loading...</p>}>
        {(cfg) => (
          <Section title="Route Bindings">
            <Index each={cfg().routing.bindings}>
              {(entry, idx) => {
                const binding = entry();
                return (
                  <div style={{ background: "var(--bg-input)", border: "1px solid var(--border-primary)", "border-radius": "var(--radius-sm)", padding: "16px", "margin-bottom": "12px" }}>
                    <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center", "margin-bottom": "12px" }}>
                      <strong style={{ color: "var(--text-heading)" }}>Route #{idx + 1}</strong>
                      <button class={styles.btnDanger} style={{ padding: "4px 12px", "font-size": "12px" }} onClick={() => updateConfig((c) => { c.routing.bindings.splice(idx, 1); return c; })}>Remove</button>
                    </div>
                    <TextInput label="Agent ID" value={binding.agent_id} onChange={(v) => updateConfig((c) => { c.routing.bindings[idx].agent_id = v; return c; })} placeholder="default" />
                    <TextInput label="Match Channel" value={binding.match_channel ?? ""} onChange={(v) => updateConfig((c) => { c.routing.bindings[idx].match_channel = v || undefined; return c; })} placeholder="telegram, discord, slack..." />
                    <TextInput label="Match Account" value={binding.match_account ?? ""} onChange={(v) => updateConfig((c) => { c.routing.bindings[idx].match_account = v || undefined; return c; })} />
                    <TextInput label="Match Peer" value={binding.match_peer ?? ""} onChange={(v) => updateConfig((c) => { c.routing.bindings[idx].match_peer = v || undefined; return c; })} />
                    <TextInput label="Match Guild/Team" value={binding.match_guild ?? ""} onChange={(v) => updateConfig((c) => { c.routing.bindings[idx].match_guild = v || undefined; return c; })} />
                  </div>
                );
              }}
            </Index>
            <button class={styles.btnSecondary} onClick={() => updateConfig((c) => { c.routing.bindings.push({ agent_id: "" }); return c; })}>+ Add Route</button>
          </Section>
        )}
      </Show>

      <SaveBar dirty={dirty()} saving={saving()} onSave={save} onDiscard={discard} />
      <Show when={toast()}>{(t) => <Toast message={t().message} type={t().type} visible={true} />}</Show>
    </div>
  );
}
