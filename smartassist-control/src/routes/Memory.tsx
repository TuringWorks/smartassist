import { Show } from "solid-js";
import { useConfig } from "../lib/useConfig";
import type { MemoryProvider, EmbeddingsProvider } from "../lib/types";
import { EnumSelect, NumberInput, Section, SaveBar, Toast, PageHeader } from "../components/FormComponents";

export default function Memory() {
  const { config, dirty, saving, toast, updateConfig, save, discard } = useConfig();

  return (
    <div style={{ "padding-bottom": "80px" }}>
      <PageHeader title="Memory" subtitle="Vector storage, embeddings, and search configuration" />

      <Show when={config()} fallback={<p style={{ color: "var(--text-tertiary)" }}>Loading...</p>}>
        {(cfg) => (
          <>
            <Section title="Provider">
              <EnumSelect<MemoryProvider> label="Memory Provider" value={cfg().memory.provider} options={[{ value: "lancedb", label: "LanceDB" }, { value: "vector_only", label: "Vector Only" }]} onChange={(v) => updateConfig((c) => { c.memory.provider = v; return c; })} />
              <EnumSelect<EmbeddingsProvider> label="Embeddings Provider" value={cfg().memory.embeddings} options={[{ value: "openai", label: "OpenAI" }, { value: "google", label: "Google" }]} onChange={(v) => updateConfig((c) => { c.memory.embeddings = v; return c; })} />
            </Section>
            <Section title="Search Settings">
              <NumberInput label="Result Limit" value={cfg().memory.search.limit} min={1} max={100} onChange={(v) => updateConfig((c) => { c.memory.search.limit = v; return c; })} help="Maximum number of search results (1-100)" />
              <NumberInput label="Top K" value={cfg().memory.search.top_k} min={1} max={100} onChange={(v) => updateConfig((c) => { c.memory.search.top_k = v; return c; })} help="Number of vector search results to retrieve" />
            </Section>
          </>
        )}
      </Show>

      <SaveBar dirty={dirty()} saving={saving()} onSave={save} onDiscard={discard} />
      <Show when={toast()}>{(t) => <Toast message={t().message} type={t().type} visible={true} />}</Show>
    </div>
  );
}
