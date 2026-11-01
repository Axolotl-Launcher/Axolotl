// Scripted tasks for the preview runtime. Tool names are real entries of the
// shared launcher tool catalog; every result, version, size and outcome below is
// sample data. `{instance}`, `{version}` and `{loader}` come from the instance the
// user picked (or the first one the launcher has).
//
// Flows are matched in order, so the more specific ones come first.
export const FLOWS = [
    {
        id: 'crash',
        requiresInstance: true,
        match: /crash|won.?t (start|launch)|doesn.?t (start|launch)|closes|崩溃|闪退|打不开|启动失败|进不去/i,
        en: {
            title: 'Fix the launch crash',
            working: 'Reading the crash report…',
            reads: [
                [
                    'instance_get',
                    'Read the instance',
                    '{instance} · Minecraft {version} · {loader}',
                ],
                [
                    'logs_list',
                    'Find the latest crash report',
                    'crash-2026-10-29_22.41.17-client.txt',
                ],
                ['logs_read', 'Read the crash report', 'A Mixin failed to apply in Entity Culling'],
                [
                    'resource_history_list',
                    'Check recent changes',
                    'Entity Culling went from 1.7.2 to 1.7.4 last night',
                ],
            ],
            answer: 'The crash comes from **Entity Culling**, which was updated last night. The report stops here:\n\n```log\n[Render thread/ERROR]: Mixin apply failed entityculling.mixins.json:WorldRendererMixin\norg.spongepowered.asm.mixin.throwables.MixinApplyError\n```\n\nThat was five minutes after it moved from `1.7.2` to `1.7.4`. The old file is still in your local backups, so I can switch it back.',
            plan: {
                title: '{instance} · Crash fix',
                summary: '1 change',
                approval: 'Restore Entity Culling 1.7.2 from your local backup',
                changes: [
                    {
                        glyph: 'EC',
                        name: 'Entity Culling',
                        note: 'Roll back to the version before the update',
                        kind: 'rollback',
                        meta: 'Local backup · 1.2 MB',
                        from: '1.7.4',
                        to: '1.7.2',
                    },
                ],
                notes: ['Uses the local backup, no download', 'The backup is verified first'],
                scope: [
                    { label: 'Roll back mods', count: 1 },
                    { label: 'Remove mods', count: 0 },
                ],
            },
            progress: [
                ['Read the crash report', 'Instance, logs and recent changes'],
                ['Find the cause', 'Entity Culling 1.7.4 fails to load'],
                ['Roll back after you approve', 'Restore 1.7.2 from backup'],
            ],
            writes: [
                [
                    'resource_history_rollback',
                    'Roll back Entity Culling',
                    'Switched to 1.7.2 · install job queued',
                    1,
                ],
                ['install_job_get', 'Confirm the install', 'Entity Culling 1.7.2 is installed'],
            ],
            result: {
                title: 'Rolled back',
                text: 'Entity Culling is back on 1.7.2. Start the game to confirm the crash is gone.',
                next: ['Launch the game to check', 'Undo this change'],
            },
            check: 'The rollback job finished: Entity Culling 1.7.2 is installed.',
            followup:
                'I checked again: the crash report and change history are the same, so the plan is still to roll back Entity Culling.',
        },
        zh: {
            title: '排查启动崩溃',
            working: '正在读取崩溃报告…',
            reads: [
                ['instance_get', '读取实例', '{instance} · Minecraft {version} · {loader}'],
                ['logs_list', '查找最近的崩溃报告', 'crash-2026-10-29_22.41.17-client.txt'],
                ['logs_read', '读取崩溃报告', 'Entity Culling 的 Mixin 注入失败'],
                [
                    'resource_history_list',
                    '查看最近的变更',
                    '昨晚 Entity Culling 从 1.7.2 更新到了 1.7.4',
                ],
            ],
            answer: '崩溃来自昨晚更新的 **Entity Culling**，崩溃报告停在这里：\n\n```log\n[Render thread/ERROR]: Mixin apply failed entityculling.mixins.json:WorldRendererMixin\norg.spongepowered.asm.mixin.throwables.MixinApplyError\n```\n\n时间就在它从 `1.7.2` 更新到 `1.7.4` 的五分钟后。旧版本文件还在本地备份里，可以换回去。',
            plan: {
                title: '{instance} · 崩溃修复',
                summary: '1 项变更',
                approval: '用本地备份把 Entity Culling 换回 1.7.2',
                changes: [
                    {
                        glyph: 'EC',
                        name: 'Entity Culling',
                        note: '回滚到更新前的版本',
                        kind: 'rollback',
                        meta: '本地备份 · 1.2 MB',
                        from: '1.7.4',
                        to: '1.7.2',
                    },
                ],
                notes: ['使用本地备份，不重新下载', '执行前先校验备份'],
                scope: [
                    { label: '回滚模组', count: 1 },
                    { label: '移除模组', count: 0 },
                ],
            },
            progress: [
                ['读取崩溃报告', '实例、日志与最近的变更'],
                ['定位原因', 'Entity Culling 1.7.4 加载失败'],
                ['确认后回滚', '从备份恢复 1.7.2'],
            ],
            writes: [
                [
                    'resource_history_rollback',
                    '回滚 Entity Culling',
                    '已换回 1.7.2 · 安装任务已排队',
                    1,
                ],
                ['install_job_get', '核对安装结果', 'Entity Culling 1.7.2 已安装'],
            ],
            result: {
                title: '已回滚',
                text: 'Entity Culling 已换回 1.7.2。启动游戏后即可确认崩溃是否解决。',
                next: ['启动游戏确认一下', '撤销这次改动'],
            },
            check: '回滚任务已经完成：Entity Culling 1.7.2 已安装。',
            followup:
                '我重新核对了一遍：崩溃报告和变更记录都没有变化，方案仍是回滚 Entity Culling。',
        },
    },
    {
        id: 'world',
        requiresInstance: true,
        match: /back ?up|world|save file|备份|存档|世界/i,
        en: {
            title: 'Back up a world',
            working: 'Looking for your worlds…',
            reads: [
                ['world_list', 'List worlds', '3 worlds in {instance}'],
                ['world_get', 'Read world details', 'Mountain Cabin · Survival · Hard'],
                ['process_list', 'Check whether the game is running', '{instance} is not running'],
            ],
            answer: 'I can back up **Mountain Cabin** right away. The game isn’t running, so the world isn’t locked.\n\nThe backup is a ZIP in this instance’s backups folder. Your save itself won’t be touched.',
            plan: {
                title: 'Mountain Cabin · World backup',
                summary: '1 new file',
                approval: 'Create a ZIP backup in the backups folder of {instance}',
                changes: [
                    {
                        icon: 'globe',
                        name: 'Mountain Cabin',
                        note: 'Single-player world · {instance}',
                        kind: 'backup',
                        meta: 'About 412 MB',
                        to: 'New ZIP',
                    },
                ],
                notes: ['Saved to the instance’s backups folder', 'The world is not modified'],
                scope: [
                    { label: 'New backups', count: 1 },
                    { label: 'Changed saves', count: 0 },
                ],
            },
            progress: [
                ['Find the world', '{instance} · Mountain Cabin'],
                ['Check it can be backed up', 'The game is not running'],
                ['Back up after you approve', 'Create a ZIP'],
            ],
            writes: [
                ['world_backup', 'Back up the world', 'Backup queued', 1],
                ['operations_get', 'Confirm the backup', 'ZIP created, 412 MB'],
            ],
            result: {
                title: 'Backup created',
                text: 'Mountain Cabin is saved as a 412 MB ZIP in the backups folder of {instance}.',
                next: ['Back up my other worlds too', 'Where is the backup?'],
            },
            check: 'The backup finished: the ZIP is in the backups folder.',
            followup: 'Checked again: the game is still closed, so the world can be backed up.',
        },
        zh: {
            title: '备份世界存档',
            working: '正在查找世界…',
            reads: [
                ['world_list', '列出世界', '{instance} 中有 3 个世界'],
                ['world_get', '读取世界信息', '山间小屋 · 生存 · 困难'],
                ['process_list', '检查游戏是否在运行', '{instance} 没有在运行'],
            ],
            answer: '可以直接备份 **山间小屋**。游戏没有运行，存档没有被占用。\n\n备份会以 ZIP 形式保存到这个实例的 backups 文件夹，不会改动存档本身。',
            plan: {
                title: '山间小屋 · 世界备份',
                summary: '1 个新文件',
                approval: '在 {instance} 的 backups 文件夹创建 ZIP 备份',
                changes: [
                    {
                        icon: 'globe',
                        name: '山间小屋',
                        note: '单人世界 · {instance}',
                        kind: 'backup',
                        meta: '约 412 MB',
                        to: '新建 ZIP',
                    },
                ],
                notes: ['保存到实例的 backups 文件夹', '不修改存档'],
                scope: [
                    { label: '新建备份', count: 1 },
                    { label: '修改存档', count: 0 },
                ],
            },
            progress: [
                ['找到世界', '{instance} · 山间小屋'],
                ['检查能否备份', '游戏未运行'],
                ['确认后备份', '创建 ZIP'],
            ],
            writes: [
                ['world_backup', '备份世界', '备份任务已排队', 1],
                ['operations_get', '核对备份', 'ZIP 已创建，412 MB'],
            ],
            result: {
                title: '备份已创建',
                text: '山间小屋已保存为 412 MB 的 ZIP，位于 {instance} 的 backups 文件夹。',
                next: ['把其他世界也备份一下', '备份文件在哪里？'],
            },
            check: '备份已经完成：ZIP 在 backups 文件夹中。',
            followup: '重新核对过了：游戏仍未运行，可以备份。',
        },
    },
    {
        id: 'pack',
        requiresInstance: false,
        match: /modpack|new instance|lightweight|set up a|整合包|新建实例|新实例|轻量/i,
        en: {
            title: 'Set up a lightweight modpack',
            working: 'Finding compatible mods…',
            reads: [
                ['system_get_memory', 'Check system memory', '16 GB total, 7.8 GB free'],
                [
                    'metadata_list_loader_versions',
                    'Look up loader versions',
                    'Fabric for 1.21.1, stable',
                ],
                ['resource_search', 'Search for mods', 'Modrinth · Fabric 1.21.1 · performance'],
                [
                    'resource_list_releases',
                    'Confirm versions exist',
                    'All three have a 1.21.1 release',
                ],
            ],
            answer: 'Here’s a light survival pack built from three performance mods.\n\nYou have 7.8 GB of free memory, so giving the game 4 GB leaves headroom. All three mods come from Modrinth and support Fabric 1.21.1. Dependencies are checked after the instance exists; if any are needed I’ll ask first.',
            plan: {
                title: 'Lite Survival · New modpack',
                summary: '5 changes',
                approval: 'Create a new instance and install 3 mods',
                changes: [
                    {
                        icon: 'cube',
                        name: 'Lite Survival',
                        note: 'New instance · Fabric 1.21.1',
                        kind: 'new',
                        meta: 'Fabric Loader 0.16.9',
                        to: 'New',
                    },
                    {
                        glyph: 'S',
                        name: 'Sodium',
                        note: 'Faster rendering',
                        kind: 'install',
                        meta: 'Modrinth · 1.1 MB',
                        to: '0.6.13',
                    },
                    {
                        glyph: 'Li',
                        name: 'Lithium',
                        note: 'Faster game logic',
                        kind: 'install',
                        meta: 'Modrinth · 0.7 MB',
                        to: '0.14.8',
                    },
                    {
                        glyph: 'Fc',
                        name: 'FerriteCore',
                        note: 'Lower memory use',
                        kind: 'install',
                        meta: 'Modrinth · 0.2 MB',
                        to: '7.0.2',
                    },
                    {
                        icon: 'memory',
                        name: 'Memory',
                        note: 'Instance launch setting',
                        kind: 'setting',
                        meta: 'Applies from the next launch',
                        from: 'Global',
                        to: '4 GB',
                    },
                ],
                notes: ['Fabric · 1.21.1', 'Dependencies checked after creation'],
                scope: [
                    { label: 'New instances', count: 1 },
                    { label: 'Install mods', count: 3 },
                    { label: 'Launch settings', count: 1 },
                ],
            },
            progress: [
                ['Understand what you need', 'Memory, loader and versions'],
                ['Pick mods and plan', '3 mods, 4 GB memory'],
                ['Create after you approve', 'New instance and mods'],
            ],
            writes: [
                ['instance_create', 'Create the instance', 'Lite Survival · Fabric 1.21.1', 1],
                ['install_job_get', 'Wait for the instance install', 'Instance ready'],
                [
                    'resource_prepare_install',
                    'Check the mods',
                    'No extra dependencies, no conflicts',
                ],
                ['resource_execute_plan', 'Install the mods', 'Install job queued', 3],
                ['launch_set_memory', 'Set memory', '4096 MiB', 1],
            ],
            result: {
                title: 'Modpack ready',
                text: 'Lite Survival is created with 3 mods and 4 GB of memory.',
                next: ['Add a minimap mod', 'Launch the new instance'],
            },
            check: 'The instance exists and all three mods are installed.',
            followup:
                'Checked again: all three mods still have Fabric 1.21.1 releases, so the plan is unchanged.',
        },
        zh: {
            title: '搭建轻量整合包',
            working: '正在查找兼容的模组…',
            reads: [
                ['system_get_memory', '检查系统内存', '共 16 GB，可用 7.8 GB'],
                ['metadata_list_loader_versions', '查询加载器版本', 'Fabric 1.21.1 稳定版'],
                ['resource_search', '搜索模组', 'Modrinth · Fabric 1.21.1 · 性能类'],
                ['resource_list_releases', '确认可用版本', '三个模组都有 1.21.1 版本'],
            ],
            answer: '用三个性能模组搭一个轻量生存整合包。\n\n系统可用内存 7.8 GB，分配 4 GB 能留出余量。三个模组都来自 Modrinth，并支持 Fabric 1.21.1。依赖要等实例创建后才能检查，如有新增会先问你。',
            plan: {
                title: '轻量生存 · 新整合包',
                summary: '5 项变更',
                approval: '新建实例并安装 3 个模组',
                changes: [
                    {
                        icon: 'cube',
                        name: '轻量生存',
                        note: '新实例 · Fabric 1.21.1',
                        kind: 'new',
                        meta: 'Fabric Loader 0.16.9',
                        to: '新建',
                    },
                    {
                        glyph: 'S',
                        name: 'Sodium',
                        note: '渲染优化',
                        kind: 'install',
                        meta: 'Modrinth · 1.1 MB',
                        to: '0.6.13',
                    },
                    {
                        glyph: 'Li',
                        name: 'Lithium',
                        note: '游戏逻辑优化',
                        kind: 'install',
                        meta: 'Modrinth · 0.7 MB',
                        to: '0.14.8',
                    },
                    {
                        glyph: 'Fc',
                        name: 'FerriteCore',
                        note: '降低内存占用',
                        kind: 'install',
                        meta: 'Modrinth · 0.2 MB',
                        to: '7.0.2',
                    },
                    {
                        icon: 'memory',
                        name: '内存分配',
                        note: '实例启动设置',
                        kind: 'setting',
                        meta: '下次启动时生效',
                        from: '全局设置',
                        to: '4 GB',
                    },
                ],
                notes: ['Fabric · 1.21.1', '创建后再检查依赖'],
                scope: [
                    { label: '新建实例', count: 1 },
                    { label: '安装模组', count: 3 },
                    { label: '启动设置', count: 1 },
                ],
            },
            progress: [
                ['了解需求与环境', '内存、加载器与版本'],
                ['挑选模组，生成方案', '3 个模组，内存 4 GB'],
                ['确认后创建', '新实例与模组'],
            ],
            writes: [
                ['instance_create', '创建实例', '轻量生存 · Fabric 1.21.1', 1],
                ['install_job_get', '等待实例安装', '实例已就绪'],
                ['resource_prepare_install', '检查模组', '无新增依赖，无冲突'],
                ['resource_execute_plan', '安装模组', '安装任务已排队', 3],
                ['launch_set_memory', '设置内存', '4096 MiB', 1],
            ],
            result: {
                title: '整合包已就绪',
                text: '「轻量生存」已创建，装好 3 个模组，内存设为 4 GB。',
                next: ['再加一个小地图模组', '启动这个新实例'],
            },
            check: '实例已存在，三个模组均已安装。',
            followup: '重新核对过了：三个模组仍有 Fabric 1.21.1 版本，方案不变。',
        },
    },
    {
        id: 'optimize',
        requiresInstance: true,
        match: /lag|slow|fps|perform|smooth|stutter|update|卡顿|性能|优化|帧率|流畅|掉帧|更新/i,
        en: {
            title: 'Make the game run smoother',
            working: 'Checking compatibility…',
            reads: [
                [
                    'instance_get',
                    'Read the instance',
                    '{instance} · Minecraft {version} · {loader}',
                ],
                ['content_list', 'List installed mods', '86 items, all enabled'],
                ['logs_read', 'Read the latest log', 'No crashes or mod errors'],
                ['resource_check_updates', 'Check for compatible updates', '2 of 86 have updates'],
                [
                    'resource_prepare_update',
                    'Prepare an update plan',
                    'Sodium and Lithium · no conflicts',
                ],
                ['backup_get_config', 'Read backup settings', 'Snapshots are enabled'],
            ],
            answer: 'Two performance mods have compatible updates: **Sodium** and **Lithium**.\n\nRender distance often matters more, but there’s no tool to change it, so I won’t touch it. You can lower it in the game’s video settings.',
            plan: {
                title: '{instance} · Performance',
                summary: '2 changes · 1 for you',
                approval: 'Create a snapshot, then update 2 mods',
                changes: [
                    {
                        glyph: 'S',
                        name: 'Sodium',
                        note: 'Faster rendering',
                        kind: 'update',
                        meta: 'Modrinth · released 6 days ago',
                        from: '0.6.5',
                        to: '0.6.13',
                    },
                    {
                        glyph: 'Li',
                        name: 'Lithium',
                        note: 'Faster game logic',
                        kind: 'update',
                        meta: 'Modrinth · released 2 weeks ago',
                        from: '0.14.3',
                        to: '0.14.8',
                    },
                    {
                        icon: 'monitor',
                        name: 'Render distance',
                        note: 'In-game video setting',
                        meta: 'Options → Video Settings → Render Distance',
                        manual: true,
                    },
                ],
                notes: ['A snapshot is created first', 'No new dependencies'],
                scope: [
                    { label: 'Update mods', count: 2 },
                    { label: 'Create snapshots', count: 1 },
                    { label: 'Remove mods', count: 0 },
                ],
            },
            progress: [
                ['Understand the instance', 'Settings, mods and recent logs'],
                ['Check compatibility and plan', '2 updates, 1 manual tip'],
                ['Apply after you approve', 'Snapshot, then update'],
            ],
            writes: [
                [
                    'backup_create',
                    'Create a snapshot',
                    'Snapshot ready, restorable from backups',
                    0,
                ],
                ['resource_execute_changes', 'Update the mods', 'Install job queued', 2],
                [
                    'install_job_get',
                    'Confirm the install',
                    'Sodium 0.6.13 and Lithium 0.14.8 installed',
                ],
            ],
            result: {
                title: 'Updates installed',
                text: 'Sodium and Lithium are updated. A snapshot of {instance} was taken first, so you can roll back from backups.',
                next: ['See if it runs smoother now', 'Undo these updates'],
            },
            check: 'The snapshot exists and both mods finished installing.',
            followup:
                'I rechecked the plan: still 2 mod updates. Render distance has no tool, so change it in the game if you like.',
        },
        zh: {
            title: '让游戏运行更流畅',
            working: '正在检查兼容性…',
            reads: [
                ['instance_get', '读取实例', '{instance} · Minecraft {version} · {loader}'],
                ['content_list', '列出已安装的模组', '86 个条目，均已启用'],
                ['logs_read', '读取最近的日志', '没有崩溃或模组报错'],
                ['resource_check_updates', '检查兼容更新', '86 个中有 2 个可以更新'],
                ['resource_prepare_update', '生成更新方案', 'Sodium、Lithium · 无冲突'],
                ['backup_get_config', '读取备份设置', '已启用快照'],
            ],
            answer: '有两个性能模组可以安全更新：**Sodium** 和 **Lithium**。\n\n渲染距离往往影响更大，但没有对应的工具，我不会改动它。需要的话可以在游戏的视频设置里调低。',
            plan: {
                title: '{instance} · 性能优化',
                summary: '2 项变更 · 1 项需手动',
                approval: '先创建快照，再更新 2 个模组',
                changes: [
                    {
                        glyph: 'S',
                        name: 'Sodium',
                        note: '渲染优化',
                        kind: 'update',
                        meta: 'Modrinth · 6 天前发布',
                        from: '0.6.5',
                        to: '0.6.13',
                    },
                    {
                        glyph: 'Li',
                        name: 'Lithium',
                        note: '游戏逻辑优化',
                        kind: 'update',
                        meta: 'Modrinth · 2 周前发布',
                        from: '0.14.3',
                        to: '0.14.8',
                    },
                    {
                        icon: 'monitor',
                        name: '渲染距离',
                        note: '游戏内视频设置',
                        meta: '选项 → 视频设置 → 渲染距离',
                        manual: true,
                    },
                ],
                notes: ['先创建实例快照', '无新增依赖'],
                scope: [
                    { label: '更新模组', count: 2 },
                    { label: '创建快照', count: 1 },
                    { label: '移除模组', count: 0 },
                ],
            },
            progress: [
                ['了解当前实例', '配置、模组与最近的日志'],
                ['检查兼容性，生成方案', '2 个更新，1 条手动建议'],
                ['确认后应用', '先快照，再更新'],
            ],
            writes: [
                ['backup_create', '创建快照', '快照已完成，可在备份中恢复', 0],
                ['resource_execute_changes', '更新模组', '安装任务已排队', 2],
                ['install_job_get', '核对安装结果', 'Sodium 0.6.13、Lithium 0.14.8 已安装'],
            ],
            result: {
                title: '更新已完成',
                text: 'Sodium 和 Lithium 已更新。更新前为 {instance} 创建了快照，可以随时从备份中恢复。',
                next: ['看看现在是否流畅', '撤回这次更新'],
            },
            check: '快照已存在，两个模组都已安装完成。',
            followup:
                '重新核对了方案：仍是 2 个模组更新。渲染距离没有对应工具，需要时可在游戏内调整。',
        },
    },
]

// Requests that match no script get an honest read-only answer.
export const GENERIC = {
    en: {
        title: '',
        working: 'Reading your launcher…',
        reads: [['instance_list', 'List instances', '{count} instances']],
        readsWithInstance: [
            ['instance_get', 'Read the instance', '{instance} · Minecraft {version} · {loader}'],
        ],
        answer: 'In this preview I can walk through four sample tasks:\n\n- Fix a launch crash\n- Make the game run smoother\n- Back up a world\n- Set up a lightweight modpack\n\nTry asking for one of them. Nothing was read beyond the instance list, and nothing was changed.',
    },
    zh: {
        title: '',
        working: '正在读取启动器…',
        reads: [['instance_list', '列出实例', '共 {count} 个实例']],
        readsWithInstance: [
            ['instance_get', '读取实例', '{instance} · Minecraft {version} · {loader}'],
        ],
        answer: '这个预览可以演示四个示例任务：\n\n- 排查启动崩溃\n- 让游戏运行更流畅\n- 备份世界存档\n- 搭建轻量整合包\n\n可以试着提出其中一个。除了实例信息外没有读取其他内容，也没有做任何修改。',
    },
}

export const SAMPLE_INSTANCE = {
    en: { name: 'Morning Mist', game_version: '1.21.1', loader: 'fabric' },
    zh: { name: '晨雾', game_version: '1.21.1', loader: 'fabric' },
}

export const ATTACHMENT_STEP = {
    en: ['Read the attached file', '{name}'],
    zh: ['读取附件', '{name}'],
}

export const PICK_INSTANCE_STEP = {
    en: ['instance_list', 'Pick an instance', 'Using {instance}, the first in your list'],
    zh: ['instance_list', '选择实例', '使用列表中的第一个实例 {instance}'],
}

// The task names an instance the launcher no longer lists (`scope: 'missing'`).
export const MISSING_INSTANCE_STEP = {
    en: [
        'instance_list',
        'Look for the chosen instance',
        'It is not in your list any more, so using {instance}, the first one',
    ],
    zh: ['instance_list', '查找所选实例', '列表中已没有所选实例，改用第一个实例 {instance}'],
}

export const CHECK_STEP = {
    en: [
        'operations_get',
        'Check what was written',
        'Read the status of the queued jobs',
        'Status checked',
    ],
    zh: ['operations_get', '核对写入情况', '读取已排队任务的状态', '已核对'],
}

export const SKIPPED = {
    en: 'Skipped as you chose: {names}.',
    zh: '按你的选择跳过了：{names}。',
}

export const FAILURE = {
    en: {
        title: 'Couldn’t reach Modrinth',
        text: 'The request timed out, so nothing was changed. Check your connection and try again.',
        step: 'Request timed out',
    },
    zh: {
        title: '暂时无法连接 Modrinth',
        text: '请求超时，没有做任何修改。检查网络后可以重试。',
        step: '请求超时',
    },
}
