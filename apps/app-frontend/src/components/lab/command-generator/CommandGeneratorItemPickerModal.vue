<script setup lang="ts">
import { defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref, useTemplateRef } from 'vue'

import advancementsData from '@/lab/command-generator/data/advancements.json'
import effectsData from '@/lab/command-generator/data/effects.json'
import { getSlotDisplay } from '@/lab/recipe-generator/display'
import { buildSlotContext } from '@/lab/recipe-generator/slot-context'
import type { JavaVersionId, SlotValue } from '@/lab/recipe-generator/types'
import itemNameIndexData from '@/lab/shared/minecraft/assets/vanilla/item-name-index.json' with { type: 'json' }
import { resolveRecipeItemName } from '@/lab/shared/minecraft/item-names.ts'
import {
    loadVersionResources,
    TEXTURE_ATLAS,
    type TextureAtlas,
} from '@/lab/shared/minecraft/resources'

import CommandGeneratorItemPicker, { type CommandItemEntry } from './CommandGeneratorItemPicker.vue'

interface EffectData {
    name: string
    iconItem?: string
    displayName?: string
}

interface AdvancementData {
    id: string
    iconItem?: string
    display?: { icon?: { item?: string } }
}

interface ExtraBlockMeta {
    id: string
    nameKey: string
    /** 该方块首次出现的 Java 版本，低于此版本的实例不显示。 */
    since?: string
}

interface SourceItem {
    id: string
    name?: string
    __extraMeta?: ExtraBlockMeta
}

const props = defineProps<{
    version: string
    kind?: 'item' | 'block' | 'effect' | 'advancement'
}>()

const emit = defineEmits<{
    select: [value: string]
}>()

const { formatMessage, locale } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const entries = ref<CommandItemEntry[]>([])
const loading = ref(false)
const cachedLocale = ref('')
const atlas: TextureAtlas = TEXTURE_ATLAS

const messages = defineMessages({
    titleItem: {
        id: 'app.lab.command-generator.item-picker.title',
        defaultMessage: 'Pick an item',
    },
    titleBlock: {
        id: 'app.lab.command-generator.block-picker.title',
        defaultMessage: 'Pick a block',
    },
    titleEffect: {
        id: 'app.lab.command-generator.effect-picker.title',
        defaultMessage: 'Pick a status effect',
    },
    titleAdvancement: {
        id: 'app.lab.command-generator.advancement-picker.title',
        defaultMessage: 'Pick an advancement',
    },
})

// 仅给 1.17+ 的方块加上 since 字段
const EXTRA_BLOCKS_META: ExtraBlockMeta[] = [
    { id: 'minecraft:air', nameKey: 'app.lab.command-generator.block.air' },
    { id: 'minecraft:cave_air', nameKey: 'app.lab.command-generator.block.cave_air' },
    { id: 'minecraft:void_air', nameKey: 'app.lab.command-generator.block.void_air' },
    { id: 'minecraft:water', nameKey: 'app.lab.command-generator.block.water' },
    { id: 'minecraft:lava', nameKey: 'app.lab.command-generator.block.lava' },
    { id: 'minecraft:fire', nameKey: 'app.lab.command-generator.block.fire' },
    { id: 'minecraft:nether_portal', nameKey: 'app.lab.command-generator.block.nether_portal' },
    { id: 'minecraft:end_portal', nameKey: 'app.lab.command-generator.block.end_portal' },
    { id: 'minecraft:end_gateway', nameKey: 'app.lab.command-generator.block.end_gateway' },
    { id: 'minecraft:bubble_column', nameKey: 'app.lab.command-generator.block.bubble_column' },
    { id: 'minecraft:moving_piston', nameKey: 'app.lab.command-generator.block.moving_piston' },
    { id: 'minecraft:piston_head', nameKey: 'app.lab.command-generator.block.piston_head' },
    { id: 'minecraft:frosted_ice', nameKey: 'app.lab.command-generator.block.frosted_ice' },
    { id: 'minecraft:light', nameKey: 'app.lab.command-generator.block.light', since: '1.17' },
]

// ================= 依赖语言文件的方块判断 =================
const itemNameIndex = itemNameIndexData as {
    en_us: Record<string, string>
    zh_cn: Record<string, string>
}

function isBlockItemId(id: string): boolean {
    const normalized = id.trim().toLowerCase()
    const namespaced = normalized.includes(':') ? normalized : `minecraft:${normalized}`
    if (!namespaced.startsWith('minecraft:') || /:\d+$/.test(namespaced)) return false
    const path = namespaced.slice('minecraft:'.length)
    if (!path) return false
    const blockKey = `block.minecraft.${path}`
    return blockKey in itemNameIndex.en_us || blockKey in itemNameIndex.zh_cn
}
// ==========================================================

function isVersionAtLeast(current: string, target: string): boolean {
    const cur = current.split('.').map(Number)
    const tgt = target.split('.').map(Number)
    const len = Math.max(cur.length, tgt.length)
    for (let i = 0; i < len; i++) {
        const c = cur[i] || 0
        const t = tgt[i] || 0
        if (c !== t) return c > t
    }
    return true
}

async function show() {
    modal.value?.show()
    if (entries.value.length > 0 && cachedLocale.value === locale.value) return

    loading.value = true
    try {
        const version = props.version as JavaVersionId
        const resources = await loadVersionResources(version)
        const ctx = buildSlotContext([], [], resources)
        const currentLocale = locale.value
        const anyRes = resources as Record<string, unknown> & {
            lang?: Record<string, string>
            effectsByName?: Record<string, EffectData>
            effects?: Record<string, EffectData>
            advancements?: Record<string, AdvancementData>
        }

        const getI18nOrLang = (key: string, langKey: string, fallback: string) => {
            const translated = formatMessage({ id: key, defaultMessage: '' })
            if (translated) return translated
            return anyRes.lang?.[langKey] || fallback
        }

        // 1. 效果
        if (props.kind === 'effect') {
            let effectList: EffectData[] = []
            const effectSource = anyRes.effectsByName || anyRes.effects
            if (effectSource) {
                effectList = Object.values(effectSource || {}) as EffectData[]
            } else {
                effectList = effectsData
                    .filter((e) => isVersionAtLeast(props.version, e.since))
                    .map((e) => ({ name: e.id, iconItem: e.icon }))
            }

            entries.value = effectList.map((effect) => {
                const shortId = effect.name
                const iconItem = effect.iconItem || 'potion'
                const fullId = `minecraft:${shortId}`

                let display = getSlotDisplay(
                    { kind: 'item', id: `minecraft:${iconItem}` } as unknown as SlotValue,
                    ctx,
                )
                if (!display)
                    display = getSlotDisplay(
                        { kind: 'item', id: 'minecraft:potion' } as unknown as SlotValue,
                        ctx,
                    )

                const displayName = getI18nOrLang(
                    `effect.${shortId}`,
                    `effect.${shortId}`,
                    effect.displayName || shortId,
                )
                return {
                    id: fullId,
                    name: displayName,
                    display: display || undefined,
                    value: { kind: 'item', id: fullId } as SlotValue,
                }
            })
            cachedLocale.value = currentLocale
            return
        }

        // 2. 成就
        if (props.kind === 'advancement') {
            let advList: AdvancementData[] = []
            if (anyRes.advancements) {
                advList = Object.values(anyRes.advancements || {}) as AdvancementData[]
            } else {
                advList = advancementsData
                    .filter((a) => isVersionAtLeast(props.version, a.since))
                    .map((a) => ({ id: a.id, iconItem: a.icon }))
            }

            entries.value = advList.map((adv) => {
                const iconItem = adv.iconItem || adv.display?.icon?.item || 'book'
                const fullId = adv.id.startsWith('minecraft:') ? adv.id : `minecraft:${adv.id}`

                let display = getSlotDisplay(
                    { kind: 'item', id: `minecraft:${iconItem}` } as unknown as SlotValue,
                    ctx,
                )
                if (!display)
                    display = getSlotDisplay(
                        { kind: 'item', id: 'minecraft:book' } as unknown as SlotValue,
                        ctx,
                    )

                const displayName = getI18nOrLang(
                    `advancement.${fullId}.title`,
                    `advancement.${fullId}.title`,
                    fullId.split('/').pop() || fullId,
                )
                return {
                    id: fullId,
                    name: displayName,
                    display: display || undefined,
                    value: { kind: 'item', id: fullId } as SlotValue,
                }
            })
            cachedLocale.value = currentLocale
            return
        }

        // 3. 物品/方块
        let sourceItems: SourceItem[] = resources.items as unknown as SourceItem[]
        let extraBlocks: SourceItem[] = []

        if (props.kind === 'block') {
            // 依赖 isBlockItemId 过滤常规方块
            sourceItems = (resources.items as unknown as SourceItem[]).filter((item) => {
                return item.id && isBlockItemId(item.id)
            })

            // 去重，并按 since 字段过滤硬编码的额外方块
            const seen = new Set(sourceItems.map((item) => item.id))
            extraBlocks = EXTRA_BLOCKS_META.filter((meta) => !seen.has(meta.id))
                .filter((meta) => !meta.since || isVersionAtLeast(props.version, meta.since))
                .map((meta) => ({
                    id: meta.id,
                    __extraMeta: meta,
                }))

            sourceItems = [...sourceItems, ...extraBlocks]
        }

        entries.value = sourceItems.map((item) => {
            const value: SlotValue = { kind: 'item', id: item.id }
            let display = undefined

            if (!item.__extraMeta) {
                display = getSlotDisplay(value, ctx)
                if (!display) {
                    display = getSlotDisplay(
                        { kind: 'block', id: item.id } as unknown as SlotValue,
                        ctx,
                    )
                }
            }

            let localizedName = resolveRecipeItemName(
                item.id,
                currentLocale,
                item.name || item.id.replace('minecraft:', ''),
            )

            if (item.__extraMeta) {
                localizedName = formatMessage({
                    id: item.__extraMeta.nameKey,
                    defaultMessage: item.id.split(':')[1],
                })
            }

            return {
                id: item.id,
                name: localizedName,
                display: display || undefined,
                value,
            }
        })
        cachedLocale.value = currentLocale
    } finally {
        loading.value = false
    }
}

function onPick(value: SlotValue) {
    const id = 'id' in value ? value.id : (value as { uid: string }).uid
    if (id) {
        emit('select', id)
        modal.value?.hide()
    }
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="
            formatMessage(
                kind === 'block'
                    ? messages.titleBlock
                    : kind === 'effect'
                      ? messages.titleEffect
                      : kind === 'advancement'
                        ? messages.titleAdvancement
                        : messages.titleItem,
            )
        "
        width="min(640px, calc(100vw - 2rem))"
        max-width="640px"
        scrollable
        max-content-height="min(40rem, 80vh)"
    >
        <div class="flex h-[32rem] min-h-0 min-w-0 flex-col">
            <CommandGeneratorItemPicker
                :entries="entries"
                :atlas="atlas"
                :loading="loading"
                :kind="kind"
                @pick="onPick"
            />
        </div>
    </NewModal>
</template>
