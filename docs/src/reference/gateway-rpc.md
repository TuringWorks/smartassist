# Gateway RPC Methods

The gateway exposes 46+ RPC methods for:

- Health monitoring (`health`, `status`)
- Chat interface (`chat`, `chat.history`, `chat.abort`)
- Agent management (`agent`, `agent.stream`)
- Session management (`sessions.list`, `sessions.resolve`, `sessions.patch`, `sessions.delete`)
- Model management (`models.list`)
- Configuration (`config.get`, `config.set`, `config.patch`, `config.schema`)
- Channel messaging (`send`, `send.poll`)
- Device pairing (`device.pair.list`, `device.pair.approve`, `device.pair.reject`, `device.token.rotate`, `device.token.revoke`)
- Node management (`node.list`, `node.describe`, `node.pair.request`, `node.pair.approve`, `node.pair.reject`, `node.unpair`, `node.rename`, `node.invoke`)
- Cron scheduling (`cron.list`, `cron.status`, `cron.add`, `cron.update`, `cron.remove`, `cron.run`, `cron.runs`, `wake`)
- Execution approvals (`exec.approvals.get`, `exec.approvals.set`, `exec.approval.request`, `exec.approval.resolve`)
- Skill management (`skills.status`, `skills.bins`, `skills.install`, `skills.update`)
- System operations (`system-presence`, `system-event`, `last-heartbeat`, `set-heartbeats`, `logs.tail`)
- Setup wizard (`wizard.start`, `wizard.next`, `wizard.cancel`, `wizard.status`)


See [Gateway Overview](../architecture/gateway.md) for the wire protocol and handler design.
