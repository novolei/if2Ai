# Architecture Gap Packs

这些 Pack 来自 `ARCHITECTURE.md` §7 的 Gap 与二次真相清单。它们不替代 MIG-016~MIG-023，而是用于清理 vNext 主线落地后暴露出来的旧事实源、边界漂移与 god-file 风险。

## Execution Order

| Order | Pack | Goal | Depends On |
| --- | --- | --- | --- |
| 1 | GAP-001 | session.json fact split | MIG-016, MIG-018 |
| 2 | GAP-002 | runtime contract unification | MIG-016, MIG-017 |
| 3 | GAP-003 | frontend projection single truth | MIG-003, MIG-017 |
| 4 | GAP-004 | command boundary thinning | MIG-015 |
| 5 | GAP-005 | stream task decomposition | MIG-016, MIG-022 |
| 6 | GAP-006 | memory UI read model unification | MIG-017 |
| 7 | GAP-007 | harness event-log truth cutover | MIG-023 |
| 8 | GAP-008 | contract drift guardrails | GAP-002 |

## Rule

每个 GAP Pack 必须引用 `ARCHITECTURE.md` 对应章节，并在 review 中回答：它消灭了哪一个二次真相，还是只新增了第三套状态。

