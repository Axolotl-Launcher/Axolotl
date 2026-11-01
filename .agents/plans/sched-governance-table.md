# Scheduler governance table

Shared by T0.5, T0.10 and T3.4. Generated from `scripts/axolotl/sched-policy.json` (`governance`).

| ID | Business | 形态 | Level | Signal-driven | Bounded exit |
| --- | --- | --- | --- | --- | --- |
| R1 | instance config sync loop | 常驻任务 | L1 | — | — |
| R2 | adaptive download concurrency controller | 常驻任务 | L1 | — | — |
| R3 | telemetry flush loop | 常驻任务 | L3 | — | — |
| R4 | Minecraft process monitor | 常驻任务 | L0 | — | — |
| R5 | file-system watcher event pump | 常驻任务 | L1 | — | — |
| R6 | manual download folder poll | 常驻任务 | L2 | — | — |
| R8 | server PTY output stream | 常驻任务 | L0 | — | — |
| R9 | server log tail | 常驻任务 | L0 | — | — |
| R10 | server process monitor | 常驻任务 | L0 | — | — |
| R11 | game locale indexer | 常驻任务 | L1 | — | — |
| R12 | Terracotta state poller | 常驻任务 | L2 | — | — |
| R13 | LAN broadcast and discovery | 常驻任务 | L2 | — | — |
| R14 | Hongshi server stdout/stderr reader | 常驻任务 | L2 | — | — |
| R15 | segmented download scheduler | 常驻任务 | L3 | — | — |
| R16 | native download scheduler | 常驻任务 | L3 | — | — |
| R17 | H2 range download scheduler | 常驻任务 | L3 | — | — |
| R18 | H2 download scheduler | 常驻任务 | L3 | — | — |
| R19 | authority connection scheduler | 常驻任务 | L3 | — | — |
| R20 | H2 connection pool maintenance | 常驻任务 | L1 | — | — |
| R21 | H2 connection pool maintenance (second loop) | 常驻任务 | L1 | — | — |
| R22 | endpoint probe rounds | 常驻任务 | L2 | — | — |
| R23 | install command stdout/stderr pump | 常驻任务 | L3 | — | — |
| R24 | multiplayer claim and tunnel loop | 常驻任务 | L3 | — | — |
| R25 | tunnel server | 常驻任务 | L2 | — | — |
| R26 | Windows system proxy watcher | 常驻任务 | L1 | — | — |
| R27 | proxy connectivity probe | 常驻任务 | L2 | — | — |
| R28 | MCP server accept loop | 常驻任务 | L1 | — | — |
| ACCENT | Windows accent colour watcher thread | 生产线程 | L2 | yes | yes |
| L0DRIVER | L0 executor driver thread (current_thread runtime) | 生产线程 | L0 | yes | yes |

Removed from the t12 list: R7 (friends socket loop deleted (no callers)); R29 (test-only thread item of the t12 list); R30 (test-only thread item of the t12 list).
