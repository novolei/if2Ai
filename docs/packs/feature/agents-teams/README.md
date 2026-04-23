# Agents Teams Pack Pipeline

Agents Teams 必须建立在 vNext session runtime 之上：event log、projection、permission recovery、supervisor、tool ledger、run report 未稳定前，不应直接做大 UI。

## Execution Order

| Order | Pack | Goal | Depends On |
| --- | --- | --- | --- |
| 1 | TEAM-001 | team domain contracts | MIG-016 |
| 2 | TEAM-002 | team API/projection skeleton | TEAM-001, MIG-017 |
| 3 | TEAM-003 | TeamSupervisor MVP | TEAM-001, MIG-020 |
| 4 | TEAM-004 | team-aware runtime correlation | TEAM-003, GAP-002 |
| 5 | TEAM-005 | TeamWorkspace UI | TEAM-002, TEAM-004 |
| 6 | TEAM-006 | team memory + permission policy | TEAM-004, MIG-019 |
| 7 | TEAM-007 | team tool ledger + review gates | TEAM-006, MIG-022 |
| 8 | TEAM-008 | team run report + harness | TEAM-007, MIG-023 |

## Guardrails

- Team 是独立 bounded context，不嵌进 session/project/identity。
- Team UI 只读 projection，不直接消费 raw events。
- Team events 必须进入 canonical event log。
- Team memory 默认不进入 global memory。

