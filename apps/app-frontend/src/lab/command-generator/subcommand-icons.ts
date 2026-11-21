import {
    ArrowUpDownIcon,
    BoxesIcon,
    BracesIcon,
    ChartAreaIcon,
    ChartColumnBigIcon,
    CircleUserIcon,
    CodeIcon,
    CompassIcon,
    CubeIcon,
    DatabaseIcon,
    EyeIcon,
    FilterIcon,
    FilterXIcon,
    GaugeIcon,
    GlobeIcon,
    GridIcon,
    LayersIcon,
    ListFilterIcon,
    MapIcon,
    MousePointer2Icon,
    OrganizationIcon,
    PackageIcon,
    PinIcon,
    RotateClockwiseIcon,
    RotateCounterClockwiseIcon,
    ScanEyeIcon,
    SunriseIcon,
    TimerIcon,
    UnknownIcon,
    UserPlusIcon,
    UserRoundIcon,
    UsersIcon,
    WorldIcon,
} from '@modrinth/assets'
import type { Component } from 'vue'

/**
 * 图标表示子命令作用于什么，动词与 if/unless 的正负由标签文字承担。
 *
 * `if` 与 `unless` 是同一组判断的两个方向，用「筛子 / 筛掉」区分；
 * 具体判断什么，由链卡片按选中的条件换成对应的 kind 图标。
 */
const SUBCOMMAND_ICONS: Record<string, Component> = {
    // 修饰子命令
    as: CircleUserIcon,
    at: PinIcon,
    on: OrganizationIcon,
    positioned: MapIcon,
    rotated: RotateClockwiseIcon,
    facing: CompassIcon,
    in: GlobeIcon,
    anchored: ArrowUpDownIcon,
    align: GridIcon,
    summon: UserPlusIcon,

    // 条件子命令（判断的正负）
    if: FilterIcon,
    unless: FilterXIcon,

    // 存储子命令
    store: ChartColumnBigIcon,
}

/**
 * 子命令后面跟的写法，例如 `if` 的 `biome`、`positioned` 的 `over`。
 *
 * 写法 id 只在家族内唯一（`positioned` 与 `rotated` 都有 `as`），
 * 所以先按 `<家族>.<写法>` 找，找不到再退回只看写法 id。
 */
const VARIANT_ICONS: Record<string, Component> = {
    entity: UsersIcon,
    block: CubeIcon,
    blocks: BoxesIcon,
    score: ChartColumnBigIcon,
    'score-matches': GaugeIcon,
    biome: LayersIcon,
    data: BracesIcon,
    items: PackageIcon,
    slots: PackageIcon,
    dimension: WorldIcon,
    loaded: EyeIcon,
    function: CodeIcon,
    predicate: ListFilterIcon,
    stopwatch: TimerIcon,

    'positioned.pos': MapIcon,
    'positioned.as': ScanEyeIcon,
    'positioned.over': SunriseIcon,
    'rotated.angle': RotateClockwiseIcon,
    'rotated.as': RotateCounterClockwiseIcon,
    'facing.pos': CompassIcon,
    'facing.entity': MousePointer2Icon,

    'store.score': ChartColumnBigIcon,
    'store.bossbar': ChartAreaIcon,
    'store.block': CubeIcon,
    'store.entity': UserRoundIcon,
    'store.storage': DatabaseIcon,
}

export function subcommandIcon(id: string): Component {
    return SUBCOMMAND_ICONS[id] ?? UnknownIcon
}

export function variantIcon(id: string, familyId?: string): Component {
    const scoped = familyId ? VARIANT_ICONS[`${familyId}.${id}`] : undefined
    return scoped ?? VARIANT_ICONS[id] ?? UnknownIcon
}
