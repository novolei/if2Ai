# ER-01 — Fix `setActiveModel` `auth_variant` Data Loss

> **来源**：[`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) §3 ER-01  
> **总览**：[`2026-05-05-improvements-wave1-overview.md`](2026-05-05-improvements-wave1-overview.md)  
> **追溯**：SP-F1 followup F1F-5 ([`2026-05-02-sp-f1-followups.md`](2026-05-02-sp-f1-followups.md))  
> **预计**：半天，1 PR

---

## 1. 问题

多认证 variant provider（典型：Moonshot 国内 `cn` vs Code 端 `code`）共享 `provider_id="moonshot"`，靠 `auth_variant` 区分。当用户在 UI 切换到一个 variant 模型并保存时，前端 `ActiveModel.auth_variant` 字段已存在，但后端 `model_set_active(provider_id, model_id)` 命令签名**不接收**该字段。

证据链（已扫描，2026-05-05）：

1. `src/api/models.ts:27-35` — `setActiveModel()` 仅传 `providerId, modelId`。
2. `src-tauri/src/commands/provider.rs:171-174` — `model_set_active(provider_id: String, model_id: String)`。
3. `src-tauri/src/modules/config/model_resolver.rs:297-301` — `set_active_model(provider_id, model_id)` 内部调 `select_model` + `set_role_config`，两者均无 variant 入口。
4. `src-tauri/src/modules/provider/service.rs:668-672` — `select_model` 写 `cfg.active_model.auth_variant: None`（**硬编码丢字段**）。
5. `src-tauri/src/modules/config/model_resolver.rs:349-355` — `set_role_config` 写 chat 角色快照时同样 `auth_variant: None`。
6. `src-tauri/src/modules/config/model_resolver.rs:285-289` — `get_active_model` 从 chat role 反构造时 `auth_variant: None`，使下次读出来也是 None（即使其他路径偶然写入了正确值）。

结果：保存后 `cfg.active_model.auth_variant` 永远是 `None`，多 variant provider 在重启或切换后被解析到错误的认证配置。

## 2. 目标

- `setActiveModel(providerId, modelId, authVariant?)` 端到端保留 `auth_variant`。
- round-trip 测试（set → get）证明 variant 不丢。
- 不破坏既有不带 variant 的调用方（4 处前端调用点 + 任何后端内部调用）。

## 3. 非目标

- 不修复 ER-02（4 处 raw `invoke('model_list_available')`）— 那是独立 PR。
- 不重构 `set_role_config` 接受复合 `model_ref`（保持 `provider_id/model_id` 字符串契约不变）。
- 不修改 UI（chat-ui / settings 页面在 ER-02 / GF-01 中处理）。

## 4. 设计

### 4.1 后端命令签名

```rust
#[tauri::command]
pub async fn model_set_active(
    provider_id: String,
    model_id: String,
    auth_variant: Option<String>,  // 新增
) -> Result<(), String> {
    crate::modules::config::model_resolver::ModelResolver::set_active_model(
        &provider_id,
        &model_id,
        auth_variant.as_deref(),       // 新增
    )
    .await
}
```

向后兼容：Tauri 自动把缺失字段视为 `None`；旧前端调用不会断。

### 4.2 `ModelResolver::set_active_model`

```rust
pub async fn set_active_model(
    provider_id: &str,
    model_id: &str,
    auth_variant: Option<&str>,  // 新增
) -> Result<(), String> {
    crate::modules::provider::service::select_model(provider_id, model_id, auth_variant).await?;
    let model_ref = format!("{provider_id}/{model_id}");
    Self::set_role_config_with_variant("chat", &model_ref, auth_variant).await
}
```

### 4.3 `ModelResolver::set_role_config_with_variant`（新增），保留旧 `set_role_config` shim

`set_role_config` 仍按现签名工作（`auth_variant: None`），新增 `set_role_config_with_variant(role, model_ref, auth_variant)` 让 `set_active_model` 调用。`cfg.active_model` 写入处用传入的 variant 替代硬编码 `None`。

```rust
pub async fn set_role_config(role: &str, model_ref: &str) -> Result<(), String> {
    Self::set_role_config_with_variant(role, model_ref, None).await
}

pub async fn set_role_config_with_variant(
    role: &str,
    model_ref: &str,
    auth_variant: Option<&str>,
) -> Result<(), String> {
    // ... 现有逻辑 ...
    if role == "chat" {
        cfg.active_model = Some(ModelSelection {
            provider_id: model.provider_id.clone(),
            model_id: model.model_id.clone(),
            auth_variant: auth_variant.map(str::to_string),  // 替换 None
        });
    }
    // ...
}
```

`commands/provider.rs::model_set_role_config` 中对 chat 角色的 `set_active_model` 调用同步加 `None`（保持当前行为；该路径是 settings 页面的角色配置，不通过 variant aware 的 `setActiveModel`）。

### 4.4 `provider::service::select_model`

```rust
pub async fn select_model(
    provider_id: &str,
    model_id: &str,
    auth_variant: Option<&str>,  // 新增
) -> Result<(), String> {
    config.active_model = Some(ModelSelection {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        auth_variant: auth_variant.map(str::to_string),  // 替换 None
    });
    // ...
}
```

更新所有 caller（grep 确认仅 `set_active_model` 一处）。

### 4.5 `ModelResolver::get_active_model`

修复 chat-role 反构造路径：

```rust
pub async fn get_active_model() -> Result<Option<ModelSelection>, String> {
    let config = Self::load_config().await?;
    if let Some(chat_role) = config.role_models.iter().find(|r| r.role == "chat") {
        if let Some(model_ref) = chat_role.model_ref.as_deref() {
            if let Some(model) = ModelRef::parse(model_ref) {
                // 优先从 cfg.active_model 取 auth_variant（chat role 字符串不携带 variant）
                let auth_variant = config
                    .active_model
                    .as_ref()
                    .filter(|m| m.provider_id == model.provider_id && m.model_id == model.model_id)
                    .and_then(|m| m.auth_variant.clone());
                return Ok(Some(ModelSelection {
                    provider_id: model.provider_id,
                    model_id: model.model_id,
                    auth_variant,
                }));
            }
        }
    }
    Ok(config.active_model.clone())
}
```

### 4.6 前端 `setActiveModel`

```ts
export async function setActiveModel(
  providerId: string,
  modelId: string,
  authVariant?: string,    // 新增
): Promise<void> {
  return getApiClient().call<void>('model_set_active', {
    providerId,
    modelId,
    authVariant,           // 新增（snake_case 由 Tauri 自动转换为 auth_variant）
  })
}
```

UI 调用点（chat-ui / Provider settings / Model settings）在本 PR **不动**，由 ER-02 / GF-01 在切到 facade 时一并补传。本 PR 仅打通通道。

## 5. TDD 步骤

### 5.1 失败测试（先写）

文件：`src-tauri/src/modules/config/model_resolver.rs` 的 `#[cfg(test)] mod tests`。

```rust
#[tokio::test]
async fn set_active_model_with_variant_preserves_round_trip() {
    let _guard = test_temp_dir_guard();   // 隔离 ~/.if2ai

    // 准备：保存一个带 auth_variant 的 provider config（满足 set_role_config 校验）
    let cfg = ProviderConfig {
        provider_id: "moonshot".into(),
        display_name: "Moonshot CN".into(),
        api_key: Some("k".into()),
        base_url: Some("https://api.moonshot.cn/v1".into()),
        auth_variant: Some("cn".into()),
    };
    ConfigService::new().save_provider(&cfg).await.unwrap();
    // models.json 也需含该 provider key "moonshot::cn"
    seed_models_json(&[("moonshot::cn", &["kimi-k2-0905-preview"])]).await;

    // act
    ModelResolver::set_active_model(
        "moonshot",
        "kimi-k2-0905-preview",
        Some("cn"),
    ).await.unwrap();

    // assert: get round-trip 保留 variant
    let got = ModelResolver::get_active_model().await.unwrap().unwrap();
    assert_eq!(got.provider_id, "moonshot");
    assert_eq!(got.model_id, "kimi-k2-0905-preview");
    assert_eq!(got.auth_variant.as_deref(), Some("cn"));
}

#[tokio::test]
async fn set_active_model_without_variant_unchanged_behavior() {
    let _guard = test_temp_dir_guard();
    seed_provider_and_models("ollama", None, &["qwen3:4b"]).await;
    ModelResolver::set_active_model("ollama", "qwen3:4b", None).await.unwrap();
    let got = ModelResolver::get_active_model().await.unwrap().unwrap();
    assert!(got.auth_variant.is_none());
}
```

如果 `model_resolver.rs` 当前没有 test 模块或 test temp dir 工具，需要补建（沿用 `triple_files.rs` 现有 test 风格）。

### 5.2 实施

按 §4 顺序：types 不变 → service::select_model → ModelResolver::{set_active_model, set_role_config_with_variant, get_active_model} → provider.rs 命令签名 → models.ts 前端。每步保持 cargo check 通过。

### 5.3 验证

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml model_resolver --lib
cargo test --manifest-path src-tauri/Cargo.toml provider::service --lib
npm test
npm run build:web
```

手工 smoke（如果有 Moonshot 账号）：
1. 配置 Moonshot CN provider（auth_variant="cn"）。
2. UI 选 Kimi 模型。
3. 重启应用。
4. 检查 chat 是否仍连 CN endpoint（`~/.if2ai/config.json` 中 `active_model.auth_variant` 应为 `"cn"`）。

## 6. 影响面

| 文件 | 改动类型 |
|------|----------|
| `src-tauri/src/modules/provider/service.rs` | `select_model` 签名加参数 + body 改 |
| `src-tauri/src/modules/config/model_resolver.rs` | 三个函数改 + 新增 `set_role_config_with_variant` + 新增 2 测试 |
| `src-tauri/src/commands/provider.rs` | `model_set_active` 签名加可选参数；内部 `set_active_model` 调用同步加 `None` |
| `src/api/models.ts` | `setActiveModel` 加可选第三参 |

UI 调用点（chat-ui:302、HomeScreen.tsx、ProvidersSettingsPage、ModelSettingsPage）**不动** — 它们当前都不传 variant，行为零变化；后续 ER-02 切到 facade 时再补传。

## 7. 回滚

单 PR 全部可回滚；新加参数都为 `Option`/可选，无 schema 迁移。

## 8. PR 描述（草案）

```markdown
## 改善 ID
ER-01 (docs/IMPROVEMENTS-2026-05-05.md §3)

## 变更摘要
修复 setActiveModel 静默丢失 auth_variant 字段的数据丢失 bug。
后端 model_set_active 命令签名加可选 auth_variant；ModelResolver 与 select_model
端到端保留该字段；get_active_model 从 cfg.active_model 反查 variant。

## Before / After
- 行为：保存 Moonshot CN variant 后重启 → 变成 None → 错误认证
- 修复后：variant 端到端保留

## 验证
- [x] 新增 2 个 round-trip 测试（with / without variant）
- [x] cargo fmt --check / clippy -D warnings / test 通过
- [x] npm test / build:web 通过

## 关联
- Plan: docs/superpowers/plans/2026-05-05-er01-set-active-model-auth-variant.md
- 追溯: SP-F1 F1F-5
```
