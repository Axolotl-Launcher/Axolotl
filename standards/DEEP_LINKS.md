# axolotl:// URL Scheme 规范

## 结构总览

```
axolotl://<命名空间>[/<动作>/<标识>][?<参数>]
```

- 命名空间小写；标识保持原样（URL 编码）；参数 UTF-8 编码
- 未知参数忽略；缺必填参数拒绝并提示
- **副作用类**走 `launch` / `install` / `join`（需用户确认）
- **特权写操作**走 `settings/set` / `stop`（双闸开关 + 启用时 10 秒强制阅读授权，之后直接执行，见 §6）
- **纯导航**走各命名空间或通用 `open`（仅白名单路径）
- 关闭“允许外部链接”开关后，所有 axolotl:// 仅提示不执行

---

## 1. 语义命令（副作用）

| 链接 | 说明 | 必填 | 可选 |
|---|---|---|---|
| `axolotl://launch?instance_id=<id>` | 启动实例 | `instance_id` | `server`、`singleplayer_world`（二选一） |
| `axolotl://join?instance_id=<id>&server=<addr>` | 启动并进服/进存档 | `instance_id` + (`server` 或 `singleplayer_world`) | |
| `axolotl://install?project=<id>` | 安装内容（默认 mod） | `project` | `kind=modpack/server`、`version`（精确版本） |
| `axolotl://install?version=<版本ID>` | 直接安装指定版本 | `version` | |

> **旧别名（行为完全一致，仅为兼容）：**  
> `axolotl://mod/<id>` → `install?project=<id>&kind=mod`  
> `axolotl://modpack/<id>` → `install?project=<id>&kind=modpack`  
> `axolotl://server/<id>` → `install?project=<id>&kind=server`  
> `axolotl://version/<vid>` → `install?version=<vid>`

---

## 2. 页面导航（纯跳转）

| 链接 | 落点 | 参数 |
|---|---|---|
| `axolotl://home` | 首页 | — |
| `axolotl://browse/<type>` | 发现页 | `type=mod/modpack/resourcepack/datapack/shader/plugin/server`；query 透传筛选 |
| `axolotl://favorites` | 收藏页（带 query 也一致，如 `favorites?q=x`） | — |
| `axolotl://project/<id>` | 项目页 | `tab=versions/gallery/changelog`、`version=<vid>` |
| `axolotl://library[/<子页>]` | 库 | 子页：`downloaded/modpacks/servers/custom` |
| `axolotl://instance/<id>[/<子页>]` | 实例详情 | 子页：`mods/files/files.studio/logs/worlds/screenshots/upgrade` |
| `axolotl://downloads` | 下载页 | `job=<任务ID>` 聚焦任务 |
| `axolotl://create` | 创建页 | `pack=<项目ID>`、`name=<名称>` |
| `axolotl://skins` / `axolotl://worlds` / `axolotl://screenshots` | 聚合页 | `screenshots` 可加 `instance=<id>` 过滤 |
| `axolotl://multiplayer/servers[/<id>]` | 服务器列表/详情 | — |
| `axolotl://multiplayer/rooms` | 房间页 | `invite=<口令>` 仅预填 |
| `axolotl://lab` / `axolotl://lab/<tool>` | Lab / 工具 | tool: `skin-editor/gradient-text/recipe-generator/schematic-preview/mod-translation` |
| `axolotl://lab/seed-map` | 种子地图 | 全参数透传（旧别名 `seed-map?...` 兼容） |
| `axolotl://settings` | 设置页 | `tab=<分类>`（query 或 `settings/<tab>` 路径二选一）、`entry=<搜索条目ID>` 定位 |
| `axolotl://help/drop` | 投放帮助页 | — |
| `axolotl://open?path=<路由>` | 通用跳转 | `path` 必须在白名单内（见下） |

**设置 tab 取值**：`interface, home-navigation, shortcut-settings, language-translation, ai, launch-defaults, instance-sync, java-performance, content-downloads, network-multiplayer, storage-backups, privacy-data, updates, about, logs`  
**entry**：传搜索条目 id（如 `privacy-external-scheme`）用于滚动定位。

---

## 3. 通用跳转白名单（`open?path=` 允许范围）

```
/, /browse/favorites,
/browse/<type> (type ∈ {mod,modpack,resourcepack,datapack,shader,plugin,server}),
/project/<id>[/versions|/gallery|/changelog],
/library[/downloaded|/modpacks|/servers|/custom],
/instance/<id>[/mods|/files|/files/studio|/logs|/worlds|/screenshots|/upgrade],
/downloads, /create, /skins, /worlds, /screenshots, /help/drop,
/multiplayer/servers[/<id>], /multiplayer/rooms,
/lab[/skin-editor|/gradient-text|/recipe-generator|/schematic-preview|/mod-translation|/seed-map],
/settings
```

---

## 4. 安全

- `open` 仅白名单路径，白名单外拒绝并提示
- `install`/`launch`/`join` 必须走确认弹窗/启动流程，不跳过校验
- 离线时浏览类（browse/project）自动回落 `/library`
- 文件/Studio 等页只导航不自动连接/执行
- 关闭“允许外部链接”开关后，所有 axolotl:// 仅弹警告不执行
- 特权动作（`settings/set`、`stop`）需“允许外部链接”与“允许特权链接操作”双开关同时开启；执行直接生效、不再逐次确认（授权在开关启用时一次性完成）
- 特权写操作只允许白名单键，未知键、非法值整单拒绝并提示
- `file://` `javascript:` 等伪协议直接拒绝

---

## 5. 实现备注（代码对应）

- 解析入口：`packages/app-lib/src/api/handler.rs::handle_url`
- 语义命令函数：`launch_payload` / `join_payload` / `install_payload`
- 导航归一：`open_route(path, query)` → 前端 `resolveOpenRoute` 白名单校验
- 前端分发：`apps/app-frontend/src/App.vue::handleCommand` 仅处理 `OpenRoute` / `OpenSettings` / `LaunchInstance` / `InstallVersion` / `InstallMod/Modpack/Server` / `RunMRPack` / `UpdateSettings` / `StopInstance`（后两者授权后直接执行）
- 设置开关：`PrivacySettings.vue` `allow_external_scheme`（默认 true）、`allow_privileged_scheme`（默认 false，双闸）
- 特权载荷：`CommandPayload::UpdateSettings` / `StopInstance`（`event/mod.rs`），后端只解析校验、不直接写
- 特权键表：`apps/app-frontend/src/helpers/deep-link-settings.ts`（与 `handler.rs` 白名单同步）
- 启用授权弹窗：`apps/app-frontend/src/components/ui/modal/PrivilegedConsentModal.vue`（开启开关时红色最高警告 + 10 秒倒计时锁死按钮（显示“启用(N)”）+ 勾选已了解风险；取消则开关保持关闭）
- 种子地图分享：`LabSeedMap.vue` 生成 `axolotl://lab/seed-map?...`
- 桌面快捷方式：`shortcuts/mod.rs` 生成 `axolotl://launch?instance_id=...`

---

## 6. 特权动作（写操作，启用时一次性授权）

| 链接 | 说明 | 必填 | 可选 |
|---|---|---|---|
| `axolotl://settings/set?<key>=<value>[&...]` | 修改设置（白名单键） | 至少一组键值 | 一次可传多键；重复键拒绝 |
| `axolotl://stop?instance_id=<id>` | 停止实例正在运行的游戏进程 | `instance_id` | 也支持路径式 `axolotl://stop/<id>` |

### 门禁（启用开关时强制阅读授权）

- **双闸**：`allow_external_scheme` 与 `allow_privileged_scheme` 同时开启才受理（后者默认关闭，入口：设置 > 隐私与数据 > 安全）
- **强制阅读只发生在启用开关时**：红色最高警告 + 10 秒倒计时锁死“启用”按钮（按钮显示 `启用(N)`）+ 勾选“我已了解风险”；取消则开关保持关闭
- 启用后每次执行**不再逐次确认**，直接生效并弹通知（改设置后立即刷新主题/外观/语言；`stop` 立即停止进程）
- 后端只解析校验并生成载荷，**不直接写入**；前端 `handleCommand` 直接执行 `settings_set` / `process_kill`
- 离线可用（本地写库）

### settings/set 白名单键

未知键、非法值整单拒绝；布尔取 `true|false`，数值按区间校验，文本去控制字符。

| 键 | 取值 | 说明 |
|---|---|---|
| `theme` | `dark/light/oled/system` | 主题 |
| `accent_color` | `pink/orange/green/blue/purple/system` 或 `custom:#rrggbb` | 强调色 |
| `locale` | 非空文本 ≤128 | 界面语言 |
| `default_page` | `Home/DiscoverContent/Library` | 默认主页 |
| `collapsed_navigation` `toggle_sidebar` `hide_nametag_skins_page` `nametag_uses_app_font` `advanced_rendering` `native_decorations` `auto_hide_downloads_button` `show_files_tab_in_instances` `show_worlds_tab_in_instances` `show_screenshots_tab_in_instances` `show_skin_selector_in_sidebar` `custom_window_title_enabled` `transparent_background` `transparent_background_blur` | 布尔 | 界面开关 |
| `close_behavior` | `ask/close/lightweight` | 关闭行为 |
| `log_level` | `error/warn/info/debug/trace` | 日志级别 |
| `home_layout` | `standard/minimal` | 主页布局 |
| `sidebar_instance_count` | 0–50 | 侧栏实例数 |
| `default_window_title` | 非空文本 ≤128 | 游戏窗口标题 |
| `transparent_background_opacity` `home_widget_background_opacity` `custom_background_component_opacity` | 0–100 | 透明度 |
| `custom_background_blur` | 0–40 | 背景模糊 |
| `custom_background_opacity` | 10–100 | 背景不透明度 |
| `hide_on_process_start` `enter_lightweight_mode_on_game_launch` `auto_set_java_high_performance_mode` `force_fullscreen` `maximize_window` | 布尔 | 启动行为 |
| `download_engine` | `legacy/xmcl` | 下载引擎 |
| `minecraft_metadata_source` `minecraft_file_source` `modrinth_source` `curseforge_source` `mojang_auth_source` | `auto/official_only/mirror_preferred/official_preferred` | 下载源 |
| `max_concurrent_downloads` `max_concurrent_writes` | 1–256 | 并发数 |
| `auto_concurrent_downloads` `bypass_curseforge_download_restrictions` `ignore_ssl_errors` | 布尔 | 下载行为（`ignore_ssl_errors` 有中间人风险） |
| `memory_max` | 1–1048576 | 最大内存（MB） |
| `memory_auto` `memory_optimize` | 布尔 | 自动内存 / 启动前优化 |
| `game_resolution_x` `game_resolution_y` | 1–65535 | 游戏窗口分辨率 |
| `hooks_pre_launch` `hooks_wrapper` `hooks_post_exit` | 文本 ≤1024，空串清空 | 启动钩子（可执行命令，最高风险） |
| `custom_dir` `backup_repository_path` `custom_background_path` | 文本 ≤1024，空串清空 | 目录/路径（可能触发数据迁移，最高风险） |

**永不开放**：telemetry / discord_rpc 等隐私项、代理与 AI 等密钥类、数组类（`extra_launch_args` `custom_env_vars` `home_widgets` `hidden_nav_items` `feature_flags` `terracotta_public_nodes`）、默认账户与登录态。

---

## 7. 标识获取方式

| 参数 | 含义 | 获取方式 |
|---|---|---|
| `instance_id` | 实例 ID（形如 `local:<uuid>`） | 启动器内打开实例详情页，地址栏 `/instance/<id>/...` 的 `<id>`（URL 解码后）即它；程序侧 `plugin:instance\|instance_list`（`helpers/instance.ts::list()`）枚举 |
| `project` | 项目 ID 或 slug | 项目页地址栏 `/project/<id>`；Modrinth 网页 `https://modrinth.com/<类型>/<slug>` 的 slug；CurseForge 用 `curseforge:<数字ID>`；程序侧 `get_project(id)` |
| `version` | 版本 ID | 项目页版本列表点开某版本后地址栏 `/project/<id>/version/<versionId>`；Modrinth 网页 `https://modrinth.com/<类型>/<slug>/version/<versionId>` 尾段；程序侧 `get_version(id)` |

> `axolotl://install?version=` 与 `axolotl://project/<id>?version=` 都接收版本 ID。