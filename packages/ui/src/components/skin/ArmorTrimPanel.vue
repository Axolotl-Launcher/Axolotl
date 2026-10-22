<script setup lang="ts">
import { computed, ref } from 'vue'

import { defineMessages, useVIntl } from '#ui/composables/i18n'
import {
    getArmorItemIcon,
    getArmorSlotIcon,
    getTrimMaterialIcon,
    getTrimPatternIcon,
} from '#ui/composables/skin-rendering/armor-preview-assets'
import {
    ARMOR_SLOTS,
    ARMOR_TRIM_MATERIALS,
    ARMOR_TRIM_PATTERNS,
    type ArmorMaterial,
    armorMaterialsForSlot,
    type ArmorPreviewConfig,
    type ArmorSlot,
    type ArmorTrimMaterial,
    type ArmorTrimPattern,
} from '#ui/composables/skin-rendering/armor-preview-types'

const messages = defineMessages({
    armorPiece: { id: 'skin.preview.armor.piece', defaultMessage: 'Armor piece' },
    armorMaterial: { id: 'skin.preview.armor.material', defaultMessage: 'Armor material' },
    trimPattern: { id: 'skin.preview.armor.trim-pattern', defaultMessage: 'Trim pattern' },
    trimMaterial: { id: 'skin.preview.armor.trim-material', defaultMessage: 'Trim material' },
    unequipped: { id: 'skin.preview.armor.unequipped', defaultMessage: 'Unequipped' },
    removePiece: {
        id: 'skin.preview.armor.remove-piece',
        defaultMessage: 'Remove {slot}',
    },
    removeTrim: { id: 'skin.preview.armor.remove-trim', defaultMessage: 'No armor trim' },
    slotState: {
        id: 'skin.preview.armor.slot-state',
        defaultMessage: '{slot}: {state}',
    },
    equipMaterial: {
        id: 'skin.preview.armor.equip-material',
        defaultMessage: '{material} {slot}',
    },
    helmet: { id: 'skin.preview.armor.slot.helmet', defaultMessage: 'Helmet' },
    chestplate: { id: 'skin.preview.armor.slot.chestplate', defaultMessage: 'Chestplate' },
    leggings: { id: 'skin.preview.armor.slot.leggings', defaultMessage: 'Leggings' },
    boots: { id: 'skin.preview.armor.slot.boots', defaultMessage: 'Boots' },
    leather: { id: 'skin.preview.armor.material.leather', defaultMessage: 'Leather' },
    chainmail: { id: 'skin.preview.armor.material.chainmail', defaultMessage: 'Chainmail' },
    copper: { id: 'skin.preview.armor.material.copper', defaultMessage: 'Copper' },
    gold: { id: 'skin.preview.armor.material.gold', defaultMessage: 'Gold' },
    iron: { id: 'skin.preview.armor.material.iron', defaultMessage: 'Iron' },
    diamond: { id: 'skin.preview.armor.material.diamond', defaultMessage: 'Diamond' },
    netherite: { id: 'skin.preview.armor.material.netherite', defaultMessage: 'Netherite' },
    turtle: { id: 'skin.preview.armor.material.turtle', defaultMessage: 'Turtle Shell' },
    bolt: { id: 'skin.preview.armor.trim.bolt', defaultMessage: 'Bolt Armor Trim' },
    coast: { id: 'skin.preview.armor.trim.coast', defaultMessage: 'Coast Armor Trim' },
    dune: { id: 'skin.preview.armor.trim.dune', defaultMessage: 'Dune Armor Trim' },
    eye: { id: 'skin.preview.armor.trim.eye', defaultMessage: 'Eye Armor Trim' },
    flow: { id: 'skin.preview.armor.trim.flow', defaultMessage: 'Flow Armor Trim' },
    host: { id: 'skin.preview.armor.trim.host', defaultMessage: 'Host Armor Trim' },
    raiser: { id: 'skin.preview.armor.trim.raiser', defaultMessage: 'Raiser Armor Trim' },
    rib: { id: 'skin.preview.armor.trim.rib', defaultMessage: 'Rib Armor Trim' },
    sentry: { id: 'skin.preview.armor.trim.sentry', defaultMessage: 'Sentry Armor Trim' },
    shaper: { id: 'skin.preview.armor.trim.shaper', defaultMessage: 'Shaper Armor Trim' },
    silence: { id: 'skin.preview.armor.trim.silence', defaultMessage: 'Silence Armor Trim' },
    snout: { id: 'skin.preview.armor.trim.snout', defaultMessage: 'Snout Armor Trim' },
    spire: { id: 'skin.preview.armor.trim.spire', defaultMessage: 'Spire Armor Trim' },
    tide: { id: 'skin.preview.armor.trim.tide', defaultMessage: 'Tide Armor Trim' },
    vex: { id: 'skin.preview.armor.trim.vex', defaultMessage: 'Vex Armor Trim' },
    ward: { id: 'skin.preview.armor.trim.ward', defaultMessage: 'Ward Armor Trim' },
    wayfinder: { id: 'skin.preview.armor.trim.wayfinder', defaultMessage: 'Wayfinder Armor Trim' },
    wild: { id: 'skin.preview.armor.trim.wild', defaultMessage: 'Wild Armor Trim' },
    amethyst: { id: 'skin.preview.armor.trim-material.amethyst', defaultMessage: 'Amethyst' },
    emerald: { id: 'skin.preview.armor.trim-material.emerald', defaultMessage: 'Emerald' },
    lapis: { id: 'skin.preview.armor.trim-material.lapis', defaultMessage: 'Lapis Lazuli' },
    quartz: { id: 'skin.preview.armor.trim-material.quartz', defaultMessage: 'Quartz' },
    redstone: { id: 'skin.preview.armor.trim-material.redstone', defaultMessage: 'Redstone' },
    resin: { id: 'skin.preview.armor.trim-material.resin', defaultMessage: 'Resin Brick' },
})

const model = defineModel<ArmorPreviewConfig>({ required: true })
const { formatMessage } = useVIntl()
const selectedSlot = ref<ArmorSlot>('helmet')
const selectedPiece = computed(() => model.value[selectedSlot.value])

function slotLabel(slot: ArmorSlot): string {
    return formatMessage(messages[slot])
}

function materialLabel(material: ArmorMaterial | ArmorTrimMaterial): string {
    return formatMessage(messages[material])
}

function patternLabel(pattern: ArmorTrimPattern): string {
    return formatMessage(messages[pattern])
}

function slotTooltip(slot: ArmorSlot): string {
    const material = model.value[slot].material
    return formatMessage(messages.slotState, {
        slot: slotLabel(slot),
        state: material ? materialLabel(material) : formatMessage(messages.unequipped),
    })
}

function materialTooltip(material: ArmorMaterial): string {
    return formatMessage(messages.equipMaterial, {
        material: materialLabel(material),
        slot: slotLabel(selectedSlot.value),
    })
}

function updateSelectedPiece(patch: Partial<ArmorPreviewConfig[ArmorSlot]>): void {
    model.value = {
        ...model.value,
        [selectedSlot.value]: {
            ...model.value[selectedSlot.value],
            ...patch,
        },
    }
}

function setMaterial(material: ArmorMaterial | null): void {
    updateSelectedPiece({ material })
}

function setTrimPattern(trimPattern: ArmorTrimPattern | null): void {
    updateSelectedPiece({ trimPattern })
}

function setTrimMaterial(trimMaterial: ArmorTrimMaterial): void {
    updateSelectedPiece({ trimMaterial })
}
</script>

<template>
    <div class="box-border min-w-0 w-full">
        <section class="armor-preview-section">
            <h3>{{ formatMessage(messages.armorPiece) }}</h3>
            <div data-tooltip-group="armor-slots" class="flex flex-wrap gap-1.5">
                <button
                    v-for="slot in ARMOR_SLOTS"
                    :key="slot"
                    v-tooltip="slotTooltip(slot)"
                    class="armor-preview-option"
                    :class="{ 'armor-preview-option--selected': selectedSlot === slot }"
                    :aria-label="slotTooltip(slot)"
                    :aria-pressed="selectedSlot === slot"
                    @click="selectedSlot = slot"
                >
                    <img
                        alt=""
                        :src="
                            model[slot].material
                                ? getArmorItemIcon(model[slot].material, slot)
                                : getArmorSlotIcon(slot)
                        "
                    />
                </button>
            </div>
        </section>

        <section class="armor-preview-section">
            <h3>{{ formatMessage(messages.armorMaterial) }}</h3>
            <div data-tooltip-group="armor-materials" class="flex flex-wrap gap-1.5">
                <button
                    v-tooltip="
                        formatMessage(messages.removePiece, { slot: slotLabel(selectedSlot) })
                    "
                    class="armor-preview-option"
                    :class="{ 'armor-preview-option--selected': !selectedPiece.material }"
                    :aria-label="
                        formatMessage(messages.removePiece, { slot: slotLabel(selectedSlot) })
                    "
                    :aria-pressed="!selectedPiece.material"
                    @click="setMaterial(null)"
                >
                    <img alt="" :src="getArmorSlotIcon(selectedSlot)" />
                </button>
                <button
                    v-for="material in armorMaterialsForSlot(selectedSlot)"
                    :key="material"
                    v-tooltip="materialTooltip(material)"
                    class="armor-preview-option"
                    :class="{
                        'armor-preview-option--selected': selectedPiece.material === material,
                    }"
                    :aria-label="materialTooltip(material)"
                    :aria-pressed="selectedPiece.material === material"
                    @click="setMaterial(material)"
                >
                    <img alt="" :src="getArmorItemIcon(material, selectedSlot)" />
                </button>
            </div>
        </section>

        <section v-if="selectedPiece.material" class="armor-preview-section">
            <h3>{{ formatMessage(messages.trimPattern) }}</h3>
            <div data-tooltip-group="armor-trim-patterns" class="flex flex-wrap gap-1.5">
                <button
                    v-tooltip="formatMessage(messages.removeTrim)"
                    class="armor-preview-option"
                    :class="{ 'armor-preview-option--selected': !selectedPiece.trimPattern }"
                    :aria-label="formatMessage(messages.removeTrim)"
                    :aria-pressed="!selectedPiece.trimPattern"
                    @click="setTrimPattern(null)"
                >
                    <img alt="" :src="getArmorSlotIcon(selectedSlot)" />
                </button>
                <button
                    v-for="pattern in ARMOR_TRIM_PATTERNS"
                    :key="pattern"
                    v-tooltip="patternLabel(pattern)"
                    class="armor-preview-option"
                    :class="{
                        'armor-preview-option--selected': selectedPiece.trimPattern === pattern,
                    }"
                    :aria-label="patternLabel(pattern)"
                    :aria-pressed="selectedPiece.trimPattern === pattern"
                    @click="setTrimPattern(pattern)"
                >
                    <img alt="" :src="getTrimPatternIcon(pattern)" />
                </button>
            </div>
        </section>

        <section
            v-if="selectedPiece.material && selectedPiece.trimPattern"
            class="armor-preview-section"
        >
            <h3>{{ formatMessage(messages.trimMaterial) }}</h3>
            <div data-tooltip-group="armor-trim-materials" class="flex flex-wrap gap-1.5">
                <button
                    v-for="material in ARMOR_TRIM_MATERIALS"
                    :key="material"
                    v-tooltip="materialLabel(material)"
                    class="armor-preview-option"
                    :class="{
                        'armor-preview-option--selected': selectedPiece.trimMaterial === material,
                    }"
                    :aria-label="materialLabel(material)"
                    :aria-pressed="selectedPiece.trimMaterial === material"
                    @click="setTrimMaterial(material)"
                >
                    <img alt="" :src="getTrimMaterialIcon(material)" />
                </button>
            </div>
        </section>
    </div>
</template>

<style scoped lang="scss">
.armor-preview-option {
    @apply shrink-0;

    box-sizing: border-box;
    height: 2.5rem;
    width: 2.5rem;
    display: grid;
    place-items: center;
    padding: 0.375rem;
    cursor: pointer;
    color: var(--color-text-default);
    background: var(--surface-3);
    border: 1px solid var(--surface-5);
    border-radius: 6px;
    box-shadow: 0 2px 5px rgb(0 0 0 / 20%);
    transition:
        background-color 120ms ease,
        border-color 120ms ease,
        transform 120ms ease;
}

.armor-preview-option:hover {
    background: var(--surface-4);
    border-color: var(--color-brand);
}

.armor-preview-option:active {
    transform: scale(0.94);
}

.armor-preview-option:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 2px;
}

.armor-preview-option--selected {
    background: var(--color-brand-highlight);
    border-color: var(--color-brand);
}

.armor-preview-option > svg {
    flex-shrink: 0;
    width: 1.35rem;
    height: 1.35rem;
}

.armor-preview-option > img {
    width: 1.75rem;
    height: 1.75rem;
    object-fit: contain;
    image-rendering: pixelated;
    filter: drop-shadow(0 2px 2px rgb(0 0 0 / 35%));
}

.armor-preview-section + .armor-preview-section {
    padding-top: 0.75rem;
    margin-top: 0.75rem;
    border-top: 1px solid var(--surface-4);
}

.armor-preview-section h3 {
    margin: 0 0 0.5rem;
    font-size: 0.75rem;
    font-weight: 600;
    line-height: 1rem;
    color: var(--color-text-tertiary);
}
</style>
