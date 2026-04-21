# Self Evolution Strategy Lifecycle Gap — 实现清单

## If2Ai 当前证据

- candidate registry / compare / gate / promote / rollback 已有
- diagnostics 页面已接真实 IPC：
  - [StrategyDiagnosticsPage.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/pages/StrategyDiagnosticsPage.tsx:1)
- 但 M5 closeout 仍明确：
  - auto trigger 未接真实 hook
  - strategy DSL 仍很有限

参考：

- [phase-m5-self-evolution-and-strategy-promotion.yaml](../../../exec-plans/active/phase-m5-self-evolution-and-strategy-promotion.yaml:281)

## 具体 Gap

1. learning governance 闭环成立
2. learning 对真实对话主链的持续收益仍弱
3. 用户看到的是诊断面，不是明显的行为进化

