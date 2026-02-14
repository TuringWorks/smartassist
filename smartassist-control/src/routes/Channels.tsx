import { Show, For } from "solid-js";
import { useConfig } from "../lib/useConfig";
import {
  TextInput,
  SecretInput,
  Toggle,
  Section,
  SaveBar,
  Toast,
  styles,
} from "../components/FormComponents";

export default function Channels() {
  const { config, dirty, saving, toast, updateConfig, save, discard } =
    useConfig();

  return (
    <div>
      <h1 style={{ "margin-bottom": "24px", "font-size": "24px" }}>
        Channels
      </h1>

      <Show when={config()} fallback={<p style={{ color: "#8b949e" }}>Loading...</p>}>
        {(cfg) => (
          <>
            {/* Telegram */}
            <Section title="Telegram" defaultOpen={!!cfg().channels.telegram}>
              <Toggle
                label="Enable Telegram"
                value={cfg().channels.telegram?.enabled ?? false}
                onChange={(v) =>
                  updateConfig((c) => {
                    if (v) {
                      c.channels.telegram = c.channels.telegram ?? {
                        enabled: true,
                        accounts: {},
                      };
                      c.channels.telegram.enabled = true;
                    } else if (c.channels.telegram) {
                      c.channels.telegram.enabled = false;
                    }
                    return c;
                  })
                }
              />
              <Show when={cfg().channels.telegram?.enabled}>
                <For
                  each={Object.entries(
                    cfg().channels.telegram?.accounts ?? {},
                  )}
                >
                  {([name, acct]) => (
                    <div
                      style={{
                        background: "#0d1117",
                        border: "1px solid #30363d",
                        "border-radius": "6px",
                        padding: "16px",
                        "margin-top": "12px",
                      }}
                    >
                      <strong style={{ color: "#f0f6fc" }}>{name}</strong>
                      <SecretInput
                        label="Bot Token"
                        value={acct.bot_token}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.telegram!.accounts[name].bot_token = v;
                            return c;
                          })
                        }
                      />
                      <TextInput
                        label="Username"
                        value={acct.username ?? ""}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.telegram!.accounts[name].username =
                              v || undefined;
                            return c;
                          })
                        }
                      />
                      <Toggle
                        label="Enabled"
                        value={acct.enabled}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.telegram!.accounts[name].enabled = v;
                            return c;
                          })
                        }
                      />
                    </div>
                  )}
                </For>
                <button
                  class={styles.btnSecondary}
                  style={{ "margin-top": "12px" }}
                  onClick={() => {
                    const name = `bot-${Date.now()}`;
                    updateConfig((c) => {
                      if (!c.channels.telegram)
                        c.channels.telegram = { enabled: true, accounts: {} };
                      c.channels.telegram.accounts[name] = {
                        bot_token: "",
                        enabled: true,
                      };
                      return c;
                    });
                  }}
                >
                  + Add Telegram Bot
                </button>
              </Show>
            </Section>

            {/* Discord */}
            <Section title="Discord" defaultOpen={!!cfg().channels.discord}>
              <Toggle
                label="Enable Discord"
                value={cfg().channels.discord?.enabled ?? false}
                onChange={(v) =>
                  updateConfig((c) => {
                    if (v) {
                      c.channels.discord = c.channels.discord ?? {
                        enabled: true,
                        accounts: {},
                      };
                      c.channels.discord.enabled = true;
                    } else if (c.channels.discord) {
                      c.channels.discord.enabled = false;
                    }
                    return c;
                  })
                }
              />
              <Show when={cfg().channels.discord?.enabled}>
                <For
                  each={Object.entries(
                    cfg().channels.discord?.accounts ?? {},
                  )}
                >
                  {([name, acct]) => (
                    <div
                      style={{
                        background: "#0d1117",
                        border: "1px solid #30363d",
                        "border-radius": "6px",
                        padding: "16px",
                        "margin-top": "12px",
                      }}
                    >
                      <strong style={{ color: "#f0f6fc" }}>{name}</strong>
                      <SecretInput
                        label="Bot Token"
                        value={acct.bot_token}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.discord!.accounts[name].bot_token = v;
                            return c;
                          })
                        }
                      />
                      <TextInput
                        label="Application ID"
                        value={acct.application_id ?? ""}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.discord!.accounts[name].application_id =
                              v || undefined;
                            return c;
                          })
                        }
                      />
                      <Toggle
                        label="Enabled"
                        value={acct.enabled}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.discord!.accounts[name].enabled = v;
                            return c;
                          })
                        }
                      />
                    </div>
                  )}
                </For>
                <button
                  class={styles.btnSecondary}
                  style={{ "margin-top": "12px" }}
                  onClick={() => {
                    const name = `bot-${Date.now()}`;
                    updateConfig((c) => {
                      if (!c.channels.discord)
                        c.channels.discord = { enabled: true, accounts: {} };
                      c.channels.discord.accounts[name] = {
                        bot_token: "",
                        enabled: true,
                      };
                      return c;
                    });
                  }}
                >
                  + Add Discord Bot
                </button>
              </Show>
            </Section>

            {/* Slack */}
            <Section title="Slack" defaultOpen={!!cfg().channels.slack}>
              <Toggle
                label="Enable Slack"
                value={cfg().channels.slack?.enabled ?? false}
                onChange={(v) =>
                  updateConfig((c) => {
                    if (v) {
                      c.channels.slack = c.channels.slack ?? {
                        enabled: true,
                        accounts: {},
                      };
                      c.channels.slack.enabled = true;
                    } else if (c.channels.slack) {
                      c.channels.slack.enabled = false;
                    }
                    return c;
                  })
                }
              />
              <Show when={cfg().channels.slack?.enabled}>
                <For
                  each={Object.entries(
                    cfg().channels.slack?.accounts ?? {},
                  )}
                >
                  {([name, acct]) => (
                    <div
                      style={{
                        background: "#0d1117",
                        border: "1px solid #30363d",
                        "border-radius": "6px",
                        padding: "16px",
                        "margin-top": "12px",
                      }}
                    >
                      <strong style={{ color: "#f0f6fc" }}>{name}</strong>
                      <SecretInput
                        label="Bot Token"
                        value={acct.bot_token}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.slack!.accounts[name].bot_token = v;
                            return c;
                          })
                        }
                      />
                      <SecretInput
                        label="App Token (Socket Mode)"
                        value={acct.app_token ?? ""}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.slack!.accounts[name].app_token =
                              v || undefined;
                            return c;
                          })
                        }
                      />
                      <Toggle
                        label="Enabled"
                        value={acct.enabled}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.slack!.accounts[name].enabled = v;
                            return c;
                          })
                        }
                      />
                    </div>
                  )}
                </For>
                <button
                  class={styles.btnSecondary}
                  style={{ "margin-top": "12px" }}
                  onClick={() => {
                    const name = `workspace-${Date.now()}`;
                    updateConfig((c) => {
                      if (!c.channels.slack)
                        c.channels.slack = { enabled: true, accounts: {} };
                      c.channels.slack.accounts[name] = {
                        bot_token: "",
                        enabled: true,
                      };
                      return c;
                    });
                  }}
                >
                  + Add Slack Workspace
                </button>
              </Show>
            </Section>

            {/* Signal */}
            <Section title="Signal" defaultOpen={!!cfg().channels.signal}>
              <Toggle
                label="Enable Signal"
                value={cfg().channels.signal?.enabled ?? false}
                onChange={(v) =>
                  updateConfig((c) => {
                    if (v) {
                      c.channels.signal = c.channels.signal ?? {
                        enabled: true,
                      };
                      c.channels.signal.enabled = true;
                    } else if (c.channels.signal) {
                      c.channels.signal.enabled = false;
                    }
                    return c;
                  })
                }
              />
              <Show when={cfg().channels.signal?.enabled}>
                <TextInput
                  label="API URL"
                  value={cfg().channels.signal?.api_url ?? ""}
                  onChange={(v) =>
                    updateConfig((c) => {
                      if (c.channels.signal)
                        c.channels.signal.api_url = v || undefined;
                      return c;
                    })
                  }
                  placeholder="http://localhost:8080"
                  help="Signal CLI REST API URL"
                />
                <TextInput
                  label="Phone Number"
                  value={cfg().channels.signal?.phone_number ?? ""}
                  onChange={(v) =>
                    updateConfig((c) => {
                      if (c.channels.signal)
                        c.channels.signal.phone_number = v || undefined;
                      return c;
                    })
                  }
                  placeholder="+1234567890"
                />
              </Show>
            </Section>

            {/* WhatsApp */}
            <Section title="WhatsApp" defaultOpen={!!cfg().channels.whatsapp}>
              <Toggle
                label="Enable WhatsApp"
                value={cfg().channels.whatsapp?.enabled ?? false}
                onChange={(v) =>
                  updateConfig((c) => {
                    if (v) {
                      c.channels.whatsapp = c.channels.whatsapp ?? {
                        enabled: true,
                        accounts: {},
                      };
                      c.channels.whatsapp.enabled = true;
                    } else if (c.channels.whatsapp) {
                      c.channels.whatsapp.enabled = false;
                    }
                    return c;
                  })
                }
              />
              <Show when={cfg().channels.whatsapp?.enabled}>
                <For
                  each={Object.entries(
                    cfg().channels.whatsapp?.accounts ?? {},
                  )}
                >
                  {([name, acct]) => (
                    <div
                      style={{
                        background: "#0d1117",
                        border: "1px solid #30363d",
                        "border-radius": "6px",
                        padding: "16px",
                        "margin-top": "12px",
                      }}
                    >
                      <strong style={{ color: "#f0f6fc" }}>{name}</strong>
                      <TextInput
                        label="Phone Number"
                        value={acct.phone_number}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.whatsapp!.accounts[name].phone_number =
                              v;
                            return c;
                          })
                        }
                      />
                      <Toggle
                        label="Enabled"
                        value={acct.enabled}
                        onChange={(v) =>
                          updateConfig((c) => {
                            c.channels.whatsapp!.accounts[name].enabled = v;
                            return c;
                          })
                        }
                      />
                    </div>
                  )}
                </For>
                <button
                  class={styles.btnSecondary}
                  style={{ "margin-top": "12px" }}
                  onClick={() => {
                    const name = `account-${Date.now()}`;
                    updateConfig((c) => {
                      if (!c.channels.whatsapp)
                        c.channels.whatsapp = { enabled: true, accounts: {} };
                      c.channels.whatsapp.accounts[name] = {
                        phone_number: "",
                        enabled: true,
                      };
                      return c;
                    });
                  }}
                >
                  + Add WhatsApp Account
                </button>
              </Show>
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
