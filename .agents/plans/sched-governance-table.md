# Scheduler governance table

Shared by T0.5, T0.10 and T3.4. Generated from `scripts/axolotl/sched-policy.json` (`governance`).

| ID | Business | Level | Signal-driven | Bounded exit |
| --- | --- | --- | --- | --- |
| R1 | instance config sync loop | L1 | — | — |
| R2 | adaptive download concurrency controller | L1 | — | — |
| R3 | telemetry flush loop | L3 | — | — |
| R4 | Minecraft process monitor | L0 | — | — |
| R5 | file-system watcher event pump | L1 | — | — |
| R6 | manual download folder poll | L2 | — | — |
| R8 | server PTY output stream | L0 | — | — |
| R9 | server log tail | L0 | — | — |
| R10 | server process monitor | L0 | — | — |
| R11 | game locale indexer | L1 | — | — |
| R12 | Terracotta state poller | L2 | — | — |
| R13 | LAN broadcast and discovery | L2 | — | — |
| R14 | Hongshi server stdout/stderr reader | L2 | — | — |
| R15 | segmented download scheduler | L3 | — | — |
| R16 | native download scheduler | L3 | — | — |
| R17 | H2 range download scheduler | L3 | — | — |
| R18 | H2 download scheduler | L3 | — | — |
| R19 | authority connection scheduler | L3 | — | — |
| R20 | H2 connection pool maintenance | L1 | — | — |
| R21 | H2 connection pool maintenance (second loop) | L1 | — | — |
| R22 | endpoint probe rounds | L2 | — | — |
| R23 | install command stdout/stderr pump | L3 | — | — |
| R24 | multiplayer claim and tunnel loop | L3 | — | — |
| R25 | tunnel server | L2 | — | — |
| R26 | Windows system proxy watcher | L1 | — | — |
| R27 | proxy connectivity probe | L2 | — | — |
| R28 | MCP server accept loop | L1 | — | — |
| ACCENT | Windows accent colour watcher thread | L2 | yes | yes |

Removed from the t12 list: R7 (friends socket loop deleted (no callers)); R29 (test-only thread item of the t12 list); R30 (test-only thread item of the t12 list).
