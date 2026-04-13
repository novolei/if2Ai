---
name: qclaw-cron-skill
description: |
  [MANDATORY - MUST LOAD] 凡是涉及「在未来某个时间点或按某种周期自动执行任务」的需求，无论用户怎样表述，都必须读取本 skill 后再操作，严禁凭记忆猜测参数。
  典型但不限于：定时任务、提醒、闹钟、倒计时、每天/每周/每月X点、X分钟后、叫我、周期执行、重复、定期、打卡、日报、周报、签到、cron、schedule、timer、remind、延迟触发、自动推送、定点通知，以及对已有定时任务的查询、暂停、恢复、取消、删除。
  本 skill 定义了 cron 内置工具与 openclaw cron CLI 的精确参数格式、时间类型判断规则、agentId 提取规则、delivery 配置，参数格式复杂且有多处 MANDATORY 约束，跳过读取将导致任务创建失败。
metadata: {"openclaw": {"emoji": "⚙️"}}
---

### cron — 定时任务

> 🚨 **[MANDATORY]** 用户提到「提醒/定时/每天X点/X分钟后/周期/重复/打卡/签到」等时，**必须创建 cron 任务**，口头承诺无效。

#### 第一步：判断创建方式

> 🚨 **[MANDATORY]** 创建定时任务**只有两种方式**，必须先判断用哪种，再决定参数。

**方式 A：调用内置 `cron` 工具**（toolCall，参数为 JSON 对象，不走 shell）

> 🚨 **[MANDATORY]** 以下场景**必须**调用内置 `cron` 工具，**严禁**通过 `exec` 工具执行 `openclaw cron add` CLI：
> - sender=`openclaw-control-ui`（本地提醒，无论是否涉及外部渠道）
> - channel=`wechat-access`（微信客服号/公众号）

内置 `cron` 工具只在本地/wechat-access session 中可用，插件可拦截并自动注入 `bestEffort`。

**方式 B：`openclaw cron add` CLI**（通过 `exec` 工具执行 shell 命令）

> 以下场景**必须**使用 CLI：
> - channel=`wecom` / `feishu` / `dingtalk-connector` / `openclaw-weixin`/ `qqbot`（外部渠道 session 中没有内置 `cron` 工具）
> - sender=`openclaw-control-ui`，且用户明确要将提醒推送到外部渠道（微信/企微/飞书/钉钉），需要从 `~/.qclaw/channel-defaults.json` 读取 `to` 时

> 🚨 **[MANDATORY]** 用户在客户端说"在微信/企微/飞书/钉钉上提醒我"等涉及任何外部渠道的提醒时，**必须先读 `~/.qclaw/channel-defaults.json`** 获取对应渠道的 `channel` 和 `to`（即 `sender_id`）。找不到对应渠道条目时才告知用户先通过该渠道给机器人发一条消息。

**渠道识别规则**（Conversation info 可能无显式 `channel` 字段，需综合判断）：
- 显式 `channel` 字段 → 直接使用
- 无 `channel` 但 `message_id` 以 `openclaw-weixin:` 开头 → 渠道为 `openclaw-weixin`
- 无 `channel` 但 `message_id` 以 `wechat-access:` 开头 → 渠道为 `wechat-access`

#### 第二步：时间类型（MANDATORY）

- 具体时刻（X点/明天X时）→ **一次性**，**禁止**创建为 `every/cron` 循环任务，执行后自动删除
- X分钟后 → **一次性**，执行后自动删除
- 周期词（每天/每小时/每X分钟）→ 周期任务
- 无明确周期词 → 默认**一次性**
- **绝对时间必须先执行 `date +%z` 获取本地时区**（`+0800` → `+08:00`），禁止硬编码，裸 ISO 按 UTC 处理会差 N 小时

#### 第三步：时间参数速查

| 用户说法 | schedule（JSON） | 命令行参数 |
|---------|-----------------|-----------|
| 每30分钟 | `{"kind":"every","everyMs":1800000}` | `--every 30m` |
| 每2小时 | `{"kind":"every","everyMs":7200000}` | `--every 2h` |
| 每天早上9点 | `{"kind":"cron","cron":"0 9 * * *"}` | `--cron "0 9 * * *"` |
| 每周一10点 | `{"kind":"cron","cron":"0 10 * * 1"}` | `--cron "0 10 * * 1"` |
| 工作日18点 | `{"kind":"cron","cron":"0 18 * * 1-5"}` | `--cron "0 18 * * 1-5"` |
| 今天下午3点（一次性） | `{"kind":"at","at":"2026-04-09T15:00:00+08:00"}` | `--at "2026-04-09T15:00:00+08:00" --delete-after-run` |
| 10分钟后（一次性） | `{"kind":"at","at":"<当前时间+10min的ISO>"}` | `--at "<当前时间+10min的ISO>" --delete-after-run` |

> cron 表达式格式：`分 时 日 月 星期`，星期 0=周日，1-5=周一至周五。

#### 方式 A：内置 `cron` 工具模板（本地 / wechat-access）

> 🚨 **[MANDATORY]** 调用的是内置 **`cron` 工具**（toolName=`cron`），**不是** `exec` 工具，**不是** shell 命令。参数为 JSON 对象直接传给工具。

> 🚨 **[MANDATORY]** `payload.message` 是定时任务触发时 agent 收到的提示词，**必须**在 message 末尾加入行为约束，防止 agent 输出内部令牌或调用错误工具：
> `要求：(1) 不要回复 HEARTBEAT_OK (2) 不要调用 message 工具 (3) 直接输出提醒文字 (4) 控制在 2-3 句话以内`

**周期任务**：
```json
{
  "action": "add",
  "job": {
    "name": "<任务名>",
    "agentId": "<agentId>",
    "schedule": {"kind":"every","everyMs":1800000},
    "sessionTarget": "isolated",
    "payload": {"kind":"agentTurn","message":"你是一个暖心的提醒助手。请用温暖、有趣的方式提醒用户：{提醒内容}。要求：(1) 不要回复 HEARTBEAT_OK (2) 不要调用 message 工具 (3) 直接输出提醒文字 (4) 控制在 2-3 句话以内"},
    "delivery": {"mode":"announce"}
  }
}
```

**一次性任务**：schedule 改为 `{"kind":"at","at":"<ISO+时区>"}`，job 中加 `"deleteAfterRun":true`

**delivery 按渠道填写：**

| 场景 | delivery 字段 |
|---|---|
| 本地提醒（无外部渠道） | `{"mode":"announce"}` |
| wechat-access | `{"mode":"announce","channel":"wechat-access","to":"<sender_id>"}` |

> `to` 从当前 Conversation info 的 `sender_id` 字段提取。
>
> 插件自动处理：无外部渠道/无 to/wechat-access 时自动注入 `bestEffort:true`；wechat-access 的 channel/to 写入 channel-defaults.json。

> 🚨 **[MANDATORY]** `agentId` **必传，禁止省略，禁止默认填 `"main"`**：
> - sessionKey 格式 `agent:【agentId】:session-xxx` → 取第二段
> - 无 sessionKey 但有 cwd（如 `/path/workspace-agent-xxx`）→ 取最后一段去掉 `workspace-` 前缀（即 `agent-xxx`），**禁止**继续去掉 `agent-` 前缀
> - 以上均无，才传 `"main"`
> - 从**当前对话**上下文提取，禁止复用历史对话的 agentId

#### 方式 B：`openclaw cron add` CLI 模板（外部渠道 / 本地→外部渠道）

> 🚨 **[MANDATORY]** `--message` 是定时任务触发时 agent 收到的提示词，**必须**在末尾加入行为约束：
> `要求：(1) 不要回复 HEARTBEAT_OK (2) 不要调用 message 工具 (3) 直接输出提醒文字 (4) 控制在 2-3 句话以内`

**周期任务**：
```bash
openclaw cron add \
  --name "<任务名>" \
  --every 30m \
  --session isolated \
  --agent <agentId> \
  --message "你是一个暖心的提醒助手。请用温暖、有趣的方式提醒用户：{提醒内容}。要求：(1) 不要回复 HEARTBEAT_OK (2) 不要调用 message 工具 (3) 直接输出提醒文字 (4) 控制在 2-3 句话以内" \
  --announce --channel <渠道> --to <sender_id>
```

**一次性任务**：将 `--every 30m` 替换为 `--at "<ISO+时区>" --delete-after-run`

- **外部渠道直发**（wecom/feishu/dingtalk-connector/openclaw-weixin）：`--channel/--to` 从 Conversation info 提取（`--to` 取 `sender_id`）
- **本地→外部渠道**：`--channel/--to` 从 `~/.qclaw/channel-defaults.json` 读取，找不到则告知先发一条消息

**`--agent` 规则**：从 sessionKey 第二段或 cwd 最后一段（去掉 `workspace-` 前缀）提取，以上均无才传 `main`。从**当前对话**上下文提取，禁止复用历史对话的 agentId。

> 🚨 **[MANDATORY] 命令失败**：最多重试一次，仍失败直接告知用户，禁止反复调试。

#### 管理命令

> 🚨 暂停/停止 ≠ 删除，"暂停/禁用"用 disable，明确说"删除"才用 remove。

**内置 `cron` 工具**：列表 `{"action":"list"}` / 暂停 `{"action":"update","jobId":"<id>","patch":{"enabled":false}}` / 恢复 `{"action":"update","jobId":"<id>","patch":{"enabled":true}}` / 删除 `{"action":"remove","jobId":"<id>"}` / 立即执行 `{"action":"run","jobId":"<id>"}`

**命令行方式**：`openclaw cron list` / `openclaw cron edit <id> --enabled false/true` / `openclaw cron remove <id>` / `openclaw cron run <id>`

#### 回复模板

一次性：`⏰ 好的，{时间}提醒你{内容}~` | 周期：`⏰ 收到，{周期}提醒你{内容}~` | 取消：`✅ 已取消"{名称}"`

> 外部渠道只输出确认话术，严禁输出推理过程。
