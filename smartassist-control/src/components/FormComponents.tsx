import { createSignal, Show, JSX } from "solid-js";
import styles from "./FormComponents.module.css";

// ── TextInput ──────────────────────────────────────────────────

interface TextInputProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  help?: string;
}

export function TextInput(props: TextInputProps) {
  return (
    <div class={styles.field}>
      <label class={styles.label}>{props.label}</label>
      <input
        class={styles.input}
        type="text"
        value={props.value}
        placeholder={props.placeholder}
        onInput={(e) => props.onChange(e.currentTarget.value)}
      />
      <Show when={props.help}>
        <div class={styles.help}>{props.help}</div>
      </Show>
    </div>
  );
}

// ── NumberInput ────────────────────────────────────────────────

interface NumberInputProps {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  help?: string;
}

export function NumberInput(props: NumberInputProps) {
  return (
    <div class={styles.field}>
      <label class={styles.label}>{props.label}</label>
      <input
        class={styles.numberInput}
        type="number"
        value={props.value}
        min={props.min}
        max={props.max}
        onInput={(e) => {
          const v = parseInt(e.currentTarget.value, 10);
          if (!isNaN(v)) props.onChange(v);
        }}
      />
      <Show when={props.help}>
        <div class={styles.help}>{props.help}</div>
      </Show>
    </div>
  );
}

// ── EnumSelect ─────────────────────────────────────────────────

interface EnumSelectProps<T extends string> {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  help?: string;
}

export function EnumSelect<T extends string>(props: EnumSelectProps<T>) {
  return (
    <div class={styles.field}>
      <label class={styles.label}>{props.label}</label>
      <select
        class={styles.select}
        value={props.value}
        onChange={(e) => props.onChange(e.currentTarget.value as T)}
      >
        {props.options.map((opt) => (
          <option value={opt.value}>{opt.label}</option>
        ))}
      </select>
      <Show when={props.help}>
        <div class={styles.help}>{props.help}</div>
      </Show>
    </div>
  );
}

// ── SecretInput ────────────────────────────────────────────────

interface SecretInputProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  help?: string;
}

export function SecretInput(props: SecretInputProps) {
  const [visible, setVisible] = createSignal(false);

  const isRedacted = () =>
    props.value === "[REDACTED]" || props.value === "***";

  return (
    <div class={styles.field}>
      <label class={styles.label}>{props.label}</label>
      <div class={styles.secretWrapper}>
        <input
          class={styles.secretInput}
          type={visible() ? "text" : "password"}
          value={isRedacted() ? "" : props.value}
          placeholder={isRedacted() ? "[REDACTED] Enter new value to change" : props.placeholder}
          onInput={(e) => props.onChange(e.currentTarget.value)}
        />
        <button
          type="button"
          class={styles.secretToggle}
          onClick={() => setVisible(!visible())}
        >
          {visible() ? "Hide" : "Show"}
        </button>
      </div>
      <Show when={props.help}>
        <div class={styles.help}>{props.help}</div>
      </Show>
    </div>
  );
}

// ── Toggle ─────────────────────────────────────────────────────

interface ToggleProps {
  label: string;
  value: boolean;
  onChange: (v: boolean) => void;
}

export function Toggle(props: ToggleProps) {
  return (
    <div
      class={styles.toggle}
      onClick={() => props.onChange(!props.value)}
    >
      <div
        class={`${styles.toggleTrack} ${props.value ? styles.toggleTrackOn : ""}`}
      >
        <div
          class={`${styles.toggleThumb} ${props.value ? styles.toggleThumbOn : ""}`}
        />
      </div>
      <span class={styles.toggleLabel}>{props.label}</span>
    </div>
  );
}

// ── Section ────────────────────────────────────────────────────

interface SectionProps {
  title: string;
  defaultOpen?: boolean;
  children: JSX.Element;
}

export function Section(props: SectionProps) {
  const [open, setOpen] = createSignal(props.defaultOpen ?? true);

  return (
    <div class={styles.section}>
      <div class={styles.sectionHeader} onClick={() => setOpen(!open())}>
        <span class={styles.sectionTitle}>{props.title}</span>
        <span
          class={`${styles.sectionArrow} ${open() ? styles.sectionArrowOpen : ""}`}
        >
          &#9654;
        </span>
      </div>
      <Show when={open()}>
        <div class={styles.sectionBody}>{props.children}</div>
      </Show>
    </div>
  );
}

// ── SaveBar ────────────────────────────────────────────────────

interface SaveBarProps {
  dirty: boolean;
  saving: boolean;
  onSave: () => void;
  onDiscard: () => void;
}

export function SaveBar(props: SaveBarProps) {
  return (
    <Show when={props.dirty}>
      <div class={styles.saveBar}>
        <button
          class={styles.btnSecondary}
          onClick={props.onDiscard}
          disabled={props.saving}
        >
          Discard
        </button>
        <button
          class={styles.btnPrimary}
          onClick={props.onSave}
          disabled={props.saving}
        >
          {props.saving ? "Saving..." : "Save Changes"}
        </button>
      </div>
    </Show>
  );
}

// ── Toast ──────────────────────────────────────────────────────

interface ToastProps {
  message: string;
  type: "success" | "error";
  visible: boolean;
}

export function Toast(props: ToastProps) {
  return (
    <Show when={props.visible}>
      <div
        class={`${styles.toast} ${props.type === "success" ? styles.toastSuccess : styles.toastError}`}
      >
        {props.message}
      </div>
    </Show>
  );
}

// ── PageHeader ─────────────────────────────────────────────────

interface PageHeaderProps {
  title: string;
  subtitle?: string;
}

export function PageHeader(props: PageHeaderProps) {
  return (
    <div class="page-header">
      <h1 class="page-title">{props.title}</h1>
      <Show when={props.subtitle}>
        <p class="page-subtitle">{props.subtitle}</p>
      </Show>
    </div>
  );
}

// ── Re-export styles for use in route pages ────────────────────

export { styles };
