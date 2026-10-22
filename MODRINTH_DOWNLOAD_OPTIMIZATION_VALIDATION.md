# Modrinth 下载优化验收记录

日期：2026-10-21。执行依据：`E:/MODRINTH_DOWNLOAD_OPTIMIZATION_PLAN.md`。

## 当前结论

实现与验收仍在进行。`AXOLOTL_MODRINTH_IP_SELECTION` 默认 `off`，不能将已通过的确定性恢复测试视为完整性能验收。尚未满足默认 `auto` 的启用门槛。

已实现 DoH 证据复用、有限 Fastly 候选规则、严格 TLS/Range 探测、共享协调器、真实 reqwest/H2 pinning、连接代次退休、同 URL 续传、连续低速 Range 恢复、扩容收益试验、缓冲输出、候选 hint 缓存及生产反馈回滚。代码和测试仍需按原计划逐项完成最终审计。

## 探测与 Modrinth 服务端统计核实

2026-10-21 阅读 [Modrinth 官方分析功能说明](https://modrinth.com/news/article/analytics-overhaul/)，该头提供下载原因、Minecraft 版本及 loader 等附加信息，没有声明省略它能禁用下载计数。公开服务端固定源码 [analytics model](https://github.com/modrinth/code/blob/7ac41ec5dc3a847c6ad0f9af69f3242588bef1b7/apps/labrinth/src/models/v3/analytics.rs) 允许附加字段为空；[AnalyticsQueue](https://github.com/modrinth/code/blob/7ac41ec5dc3a847c6ad0f9af69f3242588bef1b7/apps/labrinth/src/queue/analytics/mod.rs) 对收到的下载事件按截断 IP 与 project 合并，并在 Redis 层设有限重复计数，但不能据此推断 CDN 的 206/部分正文进入事件的具体规则。

因此只能确认本实现探测不携带下载附加分析头、Authorization、自定义敏感头及 query；不能保证额外 Range GET 绝不影响服务端下载统计。公开资料未提供明确的探测免计数契约，未联系第三方或上传用户网络数据。默认仍 off；这项服务端统计边界是默认发布前需要维护者确认的外部未验证项，不能用本地 TLS fixture 证明。

## 本轮审计修正

- 接收超时使用明确的流停滞错误，不增加 H2 协议故障评分。
- RST_STREAM 保留错误类型，只恢复对应流；连接级协议错误仍能触发回退。
- 正文截断、hash 或文件内容校验失败继续走完整性恢复，不直接推断整个 authority 的 H2 协议有问题。
- 实测更快的 HTTP/1 路径使用短期传输偏好，独立于协议失败 cooldown。
- 单一官方主机的自动测速由端点选择器负责；旧 URL 路由探测也受调节租约限制。
- 已有两个慢速窗口、10 秒最短观察时间在窗口中间满足时，不再无故多等一个完整窗口。生产首字节超时仍为 20 秒，二者是不同的计时器。
- 饱和主机只剩一个恢复槽时，候选验证串行推进，而非临时额度不足就跳过全部候选。
- 跨域跳转测试验证实际 peer 变化、普通客户端重选及 Authorization 移除。
- 显式网络上下文中的元数据请求复用该上下文，JSON 单元测试不再等待真实应用初始化。
- macOS 的系统代理模式在本优化中按路径未知处理；显式无代理和既有 Direct 路径可以参与。系统代理的原传输行为仍需跨平台验证。

后续 H2 审计统一使用最近 3 秒聚合接收窗口，替换 Range 的“连接建立以来平均值”，并用于单流同 socket 兄弟流健康保护。当前单流慢但聚合≥256KiB/s 时不触发主机换址；聚合窗口未成熟时也不作带宽结论。H2 单流及普通单流在已有选址发布后仍再次检查本地 pressure，不能拿历史慢窗口在当前磁盘忙时迁移。`%TEMP%/axolotl-recent-aggregate-tests.log` 为 314 项通过、5 项基准跳过。

主机扩容新增真实 TCP/H2 两场景实验，保留生产 3 秒窗口、两个 trial 窗口及 10% 收益门槛。两个基线 socket 各有两个活动流；第三个 socket 建立后，独立每连接 128KiB/s pacing 场景接受 target3，公共总带宽 128KiB/s 场景回退 target2、退休第三连接。实测日志 `%TEMP%/axolotl-socket-expansion-tests-fixed.log`：前者 baseline259197 B/s、12202ms 窗口内共收3977216字节，后者129926 B/s、12302ms共收1611776字节，测试通过。此实验验证真实正文计数送入调度器的反馈，不是公网 TCP 丢包或完整安装负载结论；它使用明文回环 H2，严格 TLS 另有独立恢复测试。

该状态全量下载过滤回归 `%TEMP%/axolotl-recent-aggregate-final-regression.log`：315 项通过、5 项显式基准跳过，包含真实 socket 收益实验与 H2 TLS 恢复，fmt/diff 检查通过。

新增 HTTP/1 多 Range 真实选址续传测试：8MiB 文件、慢入口45KiB/s、备选5MiB/s，生产观察时间，最终一次主机探测和一次 generation 迁移；received=unique=8388608、最终哈希正确、峰值≤8，`%TEMP%/axolotl-http1-range-peer-proof.log` 1 项通过。磁盘写入75ms或CPU90%的 selector admission 测试要求不启动 round、不消耗小时预算、不改变调节 revision，`%TEMP%/axolotl-selector-pressure-tests.log` 1 项通过。

旧内存 CDN GET 现在也先取得现有物理预算，再取得文件 semaphore；每跳选址随响应记录，重定向释放旧 hop 后再取得下一 hop 的预算。`ResponseBudget` 扩展与读取层的显式 clone 保证 reqwest 的 consuming `bytes()/bytes_stream()/json()` 在丢弃 response parts 后仍持有许可。真实慢正文测试通过 fetch_official 入口证明读取中预算少1，取消后物理和文件权重归还（只用临时内存 SQLite，没有访问应用数据库）。`%TEMP%/axolotl-memory-budget-proof.log` 1 项通过；最终下载回归 `%TEMP%/axolotl-response-budget-final-download.log` 318 项通过、5 项显式基准跳过，完整 fetch 91 项通过（`%TEMP%/axolotl-response-budget-final-fetch.log`），生产 check 通过（`%TEMP%/axolotl-response-budget-production-check.log`），fmt/diff 通过。原有 API/非 CDN 路径不因该预算改变；任意自定义客户端入口仍须按原契约审计。

## 确定性检查

以下结果分别对应其运行时的文件状态；后续修改须重跑受影响检查。

| 检查 | 结果 | 证据日志 |
|---|---|---|
| `cargo test -p theseus download -- --test-threads=1` | 293 passed，5 ignored，0 failed | `%TEMP%/axolotl-benchmark-flush-regression.log` |
| `cargo test -p theseus util::fetch::tests -- --test-threads=1` | 91 passed，0 failed | `%TEMP%/axolotl-fetch-complete-tests.log` |
| `cargo check -p theseus` | 通过 | `%TEMP%/axolotl-benchmark-stage-check.log` |
| `cargo fmt --all --check` | 通过 | 当前命令输出 |
| `git diff --check` | 通过 | 当前命令输出 |

Cargo 检查沿用配置的 target 和 profile，仅对命令进程临时设置 `CARGO_INCREMENTAL=0`，绕过此前已复现的 Rust 1.95 增量 fingerprint ICE。未清理缓存、切换 target、修改 Cargo 配置或启动桌面应用。

## 性能驱动

实现位于 `packages/app-lib/src/util/download/endpoint_selection/tests/benchmark.rs`，本地 TLS fixture 位于同目录的 `fixtures.rs`。显式 ignored 测试包含健康网络、慢入口/快备选、全部慢入口、慢尾部与 200 个混合大小文件的场景。

默认单文件规模为 64 MiB，`AXOLOTL_BENCH_SIZE_MIB` 可选择 1–128 MiB。非 64 MiB 的结果必须标为 scaled，不能声称完成 64 MiB 验收。每种单文件场景按三种模式、冷/暖连接各五轮交错执行，记录所有轮次及中位数/p95。

基准使用生产的首字节/读取超时，而非普通单元测试缩短后的超时。服务端显式 flush TLS 缓冲，使用应用层 pacing；这不是真实 TCP 丢包仿真。缓存状态仅模拟同一对象首次/后续访问，没有真实 CDN 回源成本。

`DnsOnly` 和 `Expanded` 在本地 fixture 中表示注入一组或扩展一组候选；生产 Fastly catalog 的前提与地址过滤由独立规则测试验证。这不能证明本地 IP 与真实 Fastly PoP 的对应关系。

### 完整慢网络分片运行信息

2026-10-21 15:54 左右启动 slow_fast/all_slow 各 10 个独立进程 shard；每片使用固定 64MiB、生产时序、原顺序三模式。冻结源码 `4bd746d1d`，测试二进制 SHA-256 `83FA9CC9BDEE67D56F730AC0D4DCB739D60D453FAA3E218FA8C491D11E35FC33`。日志和 PID 清单位于 `%TEMP%/axolotl-full-matrix-manifest-run2.json`，输出为 `axolotl-full-{slow_fast|all_slow}-shard-{0..9}-run2.log`，错误单独 `.err`。全部片完成并核对覆盖后方可写通过结果。

首次启动把空字符串模式环境参数显式传入，20 个进程在任何传输前因非法模式退出；保留无 run2 的日志。移除该变量后第二次正常启动，没有用失败启动数据替代性能样本。运行中同名二进制在原 target/debug/deps 内重命名为 `axolotl-download-matrix-4bd746d1d.exe`，以便后续 H2 测试重新链接，冻结文件校验值未变，原进程继续运行。最终应在进程退出后删除该临时可执行文件；未使用替代构建目录或 profile。

冻结版本下载过滤回归 313 项通过、5 项显式基准跳过（`%TEMP%/axolotl-frozen-download-regression.log`），auto_concurrency 5 项通过，生产 check 通过（`%TEMP%/axolotl-frozen-production-check.log`）。后续只对 H2 最近聚合窗口、本地压力和实测扩容继续审计，分片数据必须保留对应版本。

## 健康网络测量：保留的失败记录

命令：`cargo test -p theseus modrinth_download_benchmark_healthy -- --ignored --nocapture --test-threads=1`。

环境：本机 Windows 回环 HTTPS；64 MiB 确定性内容，目标 pacing 为 30 MiB/s。未访问公网，未使用用户代理出口进行速度排名。日志：`%TEMP%/axolotl-benchmark-healthy-flushed.log`。

| 连接 | 模式 | 轮数 | 中位完成时间 ms | p95 ms |
|---|---|---:|---:|---:|
| 冷 | off | 5 | 4310 | 4598 |
| 冷 | DNS 候选 | 5 | 4011 | 4121 |
| 冷 | 扩展候选 | 5 | 4345 | 4473 |
| 暖 | off | 5 | 2539 | 4592 |
| 暖 | DNS 候选 | 5 | 2683 | 4432 |
| 暖 | 扩展候选 | 5 | 4293 | 4555 |

所有 30 次下载均仅尝试一次，received/unique/server payload 均为每次 67,108,864 字节，probe bytes=0，IP migration=0，连接峰值=1。冷连接开启一条连接；暖连接复用一条现有连接。

冷连接 expanded 相对 off 的中位耗时增加约 0.81%，暖连接增加约 69.08%。因此测试按 5% 门槛以非零状态退出，不能记为通过。轮次存在明显波动，服务端 body_wall_time 也随之变化；目前证据不足以将其归因于调度、TLS、磁盘或选择器中的某一个因素，也不足以豁免回归。

先前两轮不采用为验收数据：第一轮继承测试超时并缺少 TLS flush，出现额外 64 KiB；第二轮改为生产超时但仍缺 flush，出现读超时重试，已停止本任务自己的该次测试进程。没有终止用户的开发或 Cargo 进程。

## 分项诊断与隔离服务端后的健康验收

前述失败之后增加了客户端 receive/output/hash，以及服务端 send/pacing 计时。共享客户端与服务端单线程执行器的诊断轮中，64 MiB 的 hash 大致为 1.0–1.1 秒，输出约为 30–43 毫秒；慢轮服务端发送耗时则从约 2.3 秒升到 3.8–4.4 秒。日志：`%TEMP%/axolotl-healthy-phase-diagnosis.log`。该轮暖连接门槛仍失败，未作为通过数据。

随后只调整本地性能 fixture：每个服务端运行在独立 OS 线程的 Tokio runtime，Drop 取消并 join 线程，服务端退出时关闭所有连接任务。生产下载器仍运行在原执行器。这样客户端同步 hash 或文件 polling 不再阻塞被模拟的远端服务端 pace 时钟。未更改 target、构建 profile 或 5% 阈值。

同样的完整 64 MiB、三模式、冷/暖各五轮交错测试通过。日志：`%TEMP%/axolotl-healthy-isolated-server.log`。

| 连接 | 模式 | 轮数 | 中位完成时间 ms | p95 ms |
|---|---|---:|---:|---:|
| 冷 | off | 5 | 2638 | 2843 |
| 冷 | DNS 候选 | 5 | 2677 | 2753 |
| 冷 | 扩展候选 | 5 | 2504 | 2649 |
| 暖 | off | 5 | 2643 | 2723 |
| 暖 | DNS 候选 | 5 | 2664 | 2790 |
| 暖 | 扩展候选 | 5 | 2618 | 2714 |

30 次均只有一次下载尝试，received/unique/server payload 均为 67,108,864 字节；probe bytes=0、migration=0、连接峰值=1。扩展候选相对 off 的冷、暖中位数分别减少约 5.08%、0.95%。这满足本地健康回归门槛，不证明公网更快，也不说明一次本地均值差异具有统计显著性。服务器隔离消除了先前明显的双峰耗时，此证据支持原测量存在执行器耦合；保留旧失败而非将其改记为成功。

## 64 MiB 慢入口到快备选：单模式测量已完成

使用生产观察时间和请求超时，慢端点为应用层 45 KiB/s pacing，快备选为 5 MiB/s。完整文件 64 MiB；`AXOLOTL_BENCH_MODE=expanded` 指定单模式，测试输出明确标记 partial mode。日志：`%TEMP%/axolotl-slow-fast-expanded-full.log`。

| 连接 | 模式 | 轮数 | 中位完成时间 ms | p95 ms | 每轮迁移 | 每轮探测轮数 | 连接峰值 |
|---|---|---:|---:|---:|---:|---:|---:|
| 冷 | 扩展候选 | 5 | 32074 | 32092 | 1 | 1 | 3 |
| 暖 | 扩展候选 | 5 | 32071 | 32082 | 1 | 1 | 3 |

每轮生产 received=unique=67,108,864 字节，最终 SHA-1 校验成功，尝试次数=2。冷、暖五轮探测正文合计分别为 43,964,416 和 43,960,320 字节，约每轮 8.8 MB。server payload 包含探测和有限 socket 预读，不能与客户端 probe 计数视为精确相等的线路字节。

本组没有实测 off/DNS 基线，不能据此声称 5 倍收益验收完成。64 MiB / 45 KiB/s 的理论耗时约 24 分钟，只用作规划运行时间。完整三模式对照仍是未完成要求。单模式开关仅供诊断，默认命令仍运行原三模式交错比较；非 64 MiB 的测试也仍标 scaled。

## 本轮预算与策略审计

- 原生 H2 双地址 TCP 竞速的第二个 socket 现在需要独立普通连接许可，不能使用保留恢复槽。无额度时按顺序推进候选，连接拒绝后仍可到达第三个地址。新增饱和主机真实 TCP 测试验证暂借额度释放。
- reqwest 的 DNS adapter 不再只返回前两个 IP；普通 DoH 的有界 provider 池也包含第三个既有 provider。新增第三地址/第三 provider 回退测试。
- 保留服务端 429/503 及其 Retry-After，并复用已有解析/上限；不再将所有探测 cooldown 都伪装成固定 429。
- off/observe 及已处于 cooldown 的调用不会取得共享调节租约，避免无网络操作却污染其他试验的 revision。
- 切换成本采用候选验证的 TTFB 作为保守冷连接开销估计（包含请求头等待，并不声称单独测出了 TCP/TLS 时间）；在 100ms–5s 内限制后用于收益判断，缺失数据保留 600ms 默认。
- 对同一 pinned client key 的并发构建做缓存二次检查，避免多个已构建 handle 各自成为独立连接池。
- 计费网络 fixture 验证实际启动的一轮在候选 DNS/正文探测前结束，probe bytes=0。

`%TEMP%/axolotl-probe-policy-budget-gate.log`：297 项通过、5 项显式基准跳过。新增第三地址/第三 provider 后，`%TEMP%/axolotl-dns-third-address-gate.log` 为 298 项通过、5 项显式基准跳过；同状态完整 fetch 为 91 项通过（`%TEMP%/axolotl-latest-fetch-tests.log`）。

随后补上 H2 等待旧连接槽时的选址竞争检查：等待者取得槽锁后重新核对 captured lease，如果代次已变化，不能分配旧连接；由有界外层重试获取当前选择。真实 TLS 测试在持有旧槽、等待请求已挂起时发布新 IP，解除锁后证明等待者使用新 peer，旧活动流仍可排空。`%TEMP%/axolotl-slot-generation-final-tests.log` 为 298 项通过、5 项显式基准跳过。

前一检查 `%TEMP%/axolotl-budget-cooldown-production-check.log` 的非测试 `cargo check -p theseus` 通过；连接槽复查后的 `%TEMP%/axolotl-slot-generation-check.log` 也通过。所有检查沿用原 target/profile，临时关闭 incremental。

## 尾段恢复修复与保留的失败证据

HEAD `4013d87df` 的完整慢尾部三模式对照跑完 30 次，测试程序退出 0，但 expanded 全部没有迁移，冷/暖中位耗时分别为 13867/13798ms。该结果不满足尾部恢复行为验收，不能用退出码代替验收。旧测速固定读取前缀，无法代表最后 512KiB 的慢速区间。原始逐轮数据已追加到 JSON，日志为 `%TEMP%/axolotl-slow-tail-full-comparison.log`。

同状态 200 文件对照在第二个 batch 失败：off 的总耗时 94050ms、连接峰值 32；DNS 模式总耗时 96865ms、峰值 33，超过 32 条上限。两批各完成 117964800 字节，received=unique，没有迁移。已保留数据，尚未解释或豁免超限；reqwest 请求许可与池中实际 socket 生命周期仍需修正。日志为 `%TEMP%/axolotl-many-files-full-comparison.log`。

本次修复把 ProbeTarget 明确为文件内的未完成半开区间，验证和吞吐请求都检查非零起止及原文件总长度。HTTP/1 单流、分段、H2 单流、H2 Range 均传递真实剩余范围。压力检查通过的触发生产窗口作为 incumbent 基准，避免再下载慢尾段两次；新候选仍需严格 TLS/Range 验证及两次一致样本。短剩余区间完成成本包含冷连接时间，不能当作饱和带宽外推。

`%TEMP%/axolotl-region-tail-proof.log` 的真实 HTTPS 64MiB 文件仅最后 512KiB 为 45KiB/s，备选 5MiB/s；使用生产观察时间，完成 11418ms、一次迁移、两次尝试、一次探测轮，probe=401408 字节、峰值 2；生产 received=unique=67108864，最终 SHA-1 正确。这是单次恢复证明，未替代五轮完整对照。新增 HTTP/1/H2 非零 Range 和溢出拒绝测试。

修复后 `%TEMP%/axolotl-region-selection-tests.log`：50 项通过、5 项显式基准跳过；`%TEMP%/axolotl-region-download-regression.log`：301 项通过、5 项显式基准跳过。`cargo fmt --all --check`、`git diff --check` 通过；先前 `cargo check -p theseus --tests` 通过，后续测试已重新编译最终代码。

本任务启动的两项旧二进制长时基准（slow-fast/all-slow）在首个完整慢基线完成前停止，以便 Windows 链接修复后的同名测试程序。保留的进度日志不是完整验收数据，完整三模式慢网络对照仍未完成。未停止用户进程，未改 target/profile。

## reqwest 空闲连接预算阶段

尾段修复后 200 文件首轮 expanded 已能完成一次选址和 15 个文件迁移，总耗时 28530ms（同轮 off 94042ms），但连接峰值 38，仍未通过。根因审计确认：普通 reqwest 最多保留 64 个空闲连接，pin 客户端也保留一个；请求结束释放许可，池中 socket 仍存活，旧/新代次及探测叠加会超出物理预算。记录 `%TEMP%/axolotl-many-files-region-comparison.log`，不把提速当作豁免上限的理由。

自动模式现在关闭 reqwest 文件客户端空闲连接保留，所有 pinned 客户端也关闭空闲池；原生 H2 保留按实际 socket 持有许可的连接池。off/observe 原客户端策略保留。reqwest 库内部竞速、异步关闭仍需审计，不能据此声称已经精确跟踪所有 socket。新增两波各 32 个真实 HTTP/1 文件、两代不同 IP 的测试，要求每波结束服务端实际 active socket 归零、峰值不超过 32。`%TEMP%/axolotl-idle-budget-tests.log`：51 项通过、5 项显式基准跳过。

完整 64MiB 健康对照再次跑完：冷 off/DNS/expanded 中位数 2658/2729/2716ms，p95 2724/2835/2832ms；暖中位数 2673/2812/2716ms，p95 2741/2925/2750ms。各组 5 次，均无探测、迁移或重复生产字节，峰值 1。expanded 冷/暖分别比 off 增加 2.18%/1.61%，测试通过其既定门槛。DNS-only 暖中位数增加 5.20%，现有测试仅对 expanded 强制门槛，故保留该未通过的子项，不称全部模式达标。本轮与批量实验并行，存在本机资源竞争；需要隔离复查该小幅超限。日志 `%TEMP%/axolotl-idle-budget-healthy.log`，原始数据已存 JSON。

关闭空闲池会增加 HTTP/1 握手成本；目前实际健康文件测量有界，但不能外推到高延迟公网或所有批量场景。完整 200 文件五轮复测尚在进行，不提前声明预算问题彻底解决。

## 尚待完成的要求

长时实验前最终生产检查 `%TEMP%/axolotl-pre-long-production-check.log` 通过；`%TEMP%/axolotl-pre-long-matrix-regression.log` 为 312 项通过、5 项显式基准跳过；完整 fetch `%TEMP%/axolotl-pre-long-fetch-final.log` 为 91 项通过；auto_concurrency 5 项通过。均沿用 target/profile、临时关闭 incremental。

主机扩容审计补上既有 3 秒 State 循环对 adaptive H2 pool 的采样：长流没有新请求取用连接时，也能结束试验、判定无收益并退休额外连接，pressure 时回退。off 的非 adaptive pool 不参与。新增真实 H2 sender/连接活动计数与注入时钟测试证明无新 checkout 也能回退；`%TEMP%/axolotl-background-pool-tests.log` 3 项通过，`cargo check -p theseus --tests` 通过（`%TEMP%/axolotl-background-pool-check.log`）。性能收益的真实每连接/公共瓶颈实验仍待补齐，不以策略测试替代。

长时基准增加只用于显式 ignored 测试的 `AXOLOTL_BENCH_SHARD=0..9`：每个 shard 为一个 iteration×冷/暖组合，内部按原三模式交错顺序执行，默认未设置时仍跑完整原矩阵。每片输出 partial 标记，只有收齐 10 片、每模式冷暖各五次才能外部汇总验收。用独立测试进程运行各片，保持每进程全局探测许可独立；本机 CPU/磁盘仍共享，必须记录该实验限制。`AXOLOTL_BENCH_SIZE_MIB=1`、shard0 的 AllSlow 调度烟测通过（`%TEMP%/axolotl-shard-scaled-smoke.log`），三模式耗时 22804/22785/22780ms、无迁移、峰值≤3、received=unique；该 scaled 单片不计完整性能验收。

重复执行 `%TEMP%/axolotl-h2-expanded-final-tests.log` 曾出现一次饱和恢复 fixture 失败：两次瞬间返回的 1MiB 样本差异过大，选择器正确拒绝晋升，但测试在 20s 超时。此前同测试通过，因此不能省略这次失败。将测试服务端的 File/LargeFile 以 32MiB/s pacing，避免把本机毫秒调度噪声当作稳定带宽；未放宽生产一致性/收益门槛。修正后的 `%TEMP%/axolotl-paced-tls-recovery-tests.log` 为 54 项通过、5 项显式基准跳过。

H2 Range 的 reqwest 回退路径现在保留 typed reqwest 错误：正文超时为 Slow，连接失败为 Connect，证书失败为 Tls，均不伪装 H2 Protocol。真实本地 206 正文停滞测试通过。原生 H2 单流、未知大小探测、资产和 H2/HTTP1 Range 响应接入现有主机 429/503 与 Retry-After 解析和冷却，沿用既有失败阈值，不随 IP/generation 重置。真实 TLS/H2 429 用例证明冷却建立后不能取得恢复 lease，原连接不退休：`%TEMP%/axolotl-native-h2-cooldown-proof.log` 1 项通过；前一版 `%TEMP%/axolotl-range-status-error-tests.log` 下载回归 311 项通过、5 项跳过，最终版正在重新验证。

显式性能矩阵新增总探测预算、最多两轮及 32 连接断言；全部慢候选中位完成耗时回归不得超过 10%，健康网络现在同时约束 DNS 与 expanded 模式的 5% 门槛。原始失败结果不会因新增断言改记为通过。

新增原生 H2 实际生产恢复证据：本地严格 TLS fixture 支持无 Range 的完整 GET、非零/开放结束 Range 与持续小块限速。单流测试为 1MiB；H2 Range 测试为完整 64MiB、显式两范围，满足生产范围并发大小限制。两者先确认慢 peer 的原生 SharedH2Connection 实际消费了正文，再由真实候选验证和两样本测速发布新 IP；最终 received=unique=文件大小、一次迁移、内容 SHA-1 正确。Range 另外要求新 peer 的原生 H2 连接实际消费正文，不能由 reqwest H2 路径代替证据。测试仅缩短观察窗口，严格 TLS/Range/预算/最终哈希沿用生产逻辑。`%TEMP%/axolotl-h2-range-full-proof.log`：54 项通过、5 项显式基准跳过。

DNS metadata 缓存阶段：provider answers 和候选快照最多各保留 128 个 host，插入时清理过期项并淘汰最早获取的缓存。候选查询锁清理无人持有的 entry；128 个活跃 host 已满时拒绝新候选查询，保留已在途的单主机合并锁，避免用淘汰活跃锁制造重复查询。普通下载 DNS 不依赖候选锁，不因候选缓存淘汰而禁用正常下载。新增 148 个 host 插入后的容量与旧项淘汰测试，以及活跃候选查询容量拒绝测试；上述 308 项回归已覆盖。

诊断与样本审计阶段：引入稳定 `EndpointReason` 枚举，区分证书、Range 不支持/边界错误/编码错误、正文截断、冷却、预算、旧 epoch、样本不足及本地压力。每轮开始/结束保留 captured epoch/generation、总耗时与本轮 probe bytes；测速日志补候选来源、目标 IP、协议、样本类型、正文时长、TTFB、throughput、污染状态与本地实际区间收益估计。没有记录 query/token/代理凭据，探测日志不替换生产最终错误。

ProbeSession 捕获同一调节 registry，每次读正文及结束检查资源压力和 revision；跨过其他调节试验的窗口标记 contaminated，不能晋升。真实 TLS 慢文件测试在测量中间插入文件并发试验，断言样本被污染。极端 u64 文件长度的 ETA 和重连相加改为饱和运算，避免错误元数据导致 Duration panic。

`%TEMP%/axolotl-diagnostics-download-tests.log`：308 项通过、5 项显式基准跳过，包含新增污染窗口、极端 ETA 及后续 DNS 缓存边界用例。`cargo check -p theseus --tests` 通过（`%TEMP%/axolotl-diagnostics-final-check.log`），最终新增测试已由 cargo test 重新编译；fmt/diff 检查通过。长时性能数据对应前一提交，后续行为影响仍按范围复查。

200 文件对照 `%TEMP%/axolotl-idle-budget-many-files.log` 全部结束：15 个 batch、3000 个完整校验文件，三模式各五轮。off 总耗时中位数 94031ms、p95 94038ms；DNS 96879/96884ms；expanded 28531/31360ms（中位耗时约 3.30 倍改善）。全部 batch 峰值不超过 32、received=unique=117964800 字节。expanded 各轮仅一轮主机探测，五轮 probe 合计 4904960 字节，共 77 个文件安全迁移；off/DNS 均无迁移，DNS 因无其他端点本轮 probe=0。原始 batch 数据已保存 JSON。本地成功不替代高 RTT 公网及更复杂 socket 竞速的验证。

健康重复轮 `%TEMP%/axolotl-idle-budget-healthy-repeat.log` 冷 off/DNS/expanded 中位数 2737/2683/2648ms，暖 2689/2670/2601ms，全部在 5% 中位回归门槛内。30 次均无 probe/migration/重复生产正文，峰值 1；原始数据保存 JSON。前一轮 DNS 暖 +5.20% 的记录保留，两轮都与批量实验共用本机，不能声称已获得隔离的公网握手成本结论。

关闭空闲池后的慢尾部完整对照已完成，日志 `%TEMP%/axolotl-region-tail-matrix.log`，30 次完整原始数据存 JSON。冷 off/DNS/expanded 中位耗时 14200/13964/11414ms，p95 14274/14186/11437ms；暖中位耗时 13931/13991/11392ms，p95 14163/14097/11409ms。expanded 每次均一次真实迁移、两次尝试、一次探测轮，冷暖各五次 probe 合计均 2156544 字节（约 431KB/次），峰值 2。所有模式每次 received=unique=67108864，SHA-1 通过。新增后续矩阵强制断言，不能再出现“没有迁移但仅完成下载就算通过”。该轮为 `a423ffdb7` 对应测试二进制，之后诊断与污染窗口审计尚待测试。

同状态完整 `util::fetch::tests` 91 项通过（`%TEMP%/axolotl-idle-budget-fetch.log`）。

普通 DoH 家族等待修复：原 `resolve_families` 用 join 等待两个家族完整超时，即使 A 已成功也可能等待 AAAA 五秒。现在首个非空成功后仅收集最多 500ms 的另一家族，仍保留 400ms 到达的双栈结果；空答案或失败不取消尚可能成功的家族。候选汇总继续使用原独立整体 deadline。`%TEMP%/axolotl-dns-family-tests.log`：36 项通过，含 IPv4/IPv6 任一成功另一方永久停滞、晚到双栈和空答案回退。`cargo check -p theseus --tests` 已通过（`%TEMP%/axolotl-dns-family-check.log`），沿用 target/profile。

- 健康网络本地回归已在隔离 fixture 后通过；最终代码变化如影响该路径，仍须按范围复查。
- 执行慢入口/快备选、全部慢入口、慢尾部及 200 文件对照矩阵；缩小规模不能替代完整规模的收益结论。
- 继续审核 H2 单流、H2 Range、HTTP/1 Range、内存下载、重定向和失败恢复的全部实际路径与计数覆盖。
- 补齐候选证据、计费网络、代理上下文、网络代次及过期结果的验收映射；记录未验证平台。
- 审核物理连接预算与库内部连接生命周期，包含竞速建连、旧代次、探测、恢复和取消。
- 检查恢复总尝试预算、探测干扰样本、正文预算和冷却是否在完整场景中按计划成立。
- 补齐稳定诊断字段与失败原因；不能用日志或策略 mock 替代真实连接证据。
- 完成真实网络可取得的验证。当前没有国内直连跨运营商提速数据，跨网络收益未验证。
- 最后按计划第 18 节逐项审计，并在满足门槛后处理默认 `auto`；此前保持 `off`。

本文件记录阶段证据和未完成项，不是整体完成声明。
