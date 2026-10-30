<script setup lang="ts">
import { defineMessages, Toggle, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { SchematicStateProperty } from '@/lab/schematic-preview/block-state'

const props = defineProps<{
    properties: SchematicStateProperty[]
    disabled?: boolean
}>()

const emit = defineEmits<{
    change: [key: string, value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    empty: {
        id: 'app.lab.schematic-preview.state.empty',
        defaultMessage: 'This block has no state properties.',
    },
    readonly: {
        id: 'app.lab.schematic-preview.state.readonly',
        defaultMessage: 'Some properties cannot be changed here.',
    },
    propertyFacing: {
        id: 'app.lab.schematic-preview.state.property.facing',
        defaultMessage: 'Facing',
    },
    propertyAxis: {
        id: 'app.lab.schematic-preview.state.property.axis',
        defaultMessage: 'Axis',
    },
    propertyHalf: {
        id: 'app.lab.schematic-preview.state.property.half',
        defaultMessage: 'Half',
    },
    propertyType: {
        id: 'app.lab.schematic-preview.state.property.type',
        defaultMessage: 'Type',
    },
    propertyShape: {
        id: 'app.lab.schematic-preview.state.property.shape',
        defaultMessage: 'Shape',
    },
    propertyFace: {
        id: 'app.lab.schematic-preview.state.property.face',
        defaultMessage: 'Face',
    },
    propertyAttachment: {
        id: 'app.lab.schematic-preview.state.property.attachment',
        defaultMessage: 'Attachment',
    },
    propertyHinge: {
        id: 'app.lab.schematic-preview.state.property.hinge',
        defaultMessage: 'Hinge',
    },
    propertyPart: {
        id: 'app.lab.schematic-preview.state.property.part',
        defaultMessage: 'Part',
    },
    propertyOrientation: {
        id: 'app.lab.schematic-preview.state.property.orientation',
        defaultMessage: 'Orientation',
    },
    propertyRotation: {
        id: 'app.lab.schematic-preview.state.property.rotation',
        defaultMessage: 'Rotation',
    },
    propertyVerticalDirection: {
        id: 'app.lab.schematic-preview.state.property.vertical_direction',
        defaultMessage: 'Vertical direction',
    },
    propertyTilt: {
        id: 'app.lab.schematic-preview.state.property.tilt',
        defaultMessage: 'Tilt',
    },
    propertyWaterlogged: {
        id: 'app.lab.schematic-preview.state.property.waterlogged',
        defaultMessage: 'Waterlogged',
    },
    propertyPowered: {
        id: 'app.lab.schematic-preview.state.property.powered',
        defaultMessage: 'Powered',
    },
    propertyLit: {
        id: 'app.lab.schematic-preview.state.property.lit',
        defaultMessage: 'Lit',
    },
    propertyOpen: {
        id: 'app.lab.schematic-preview.state.property.open',
        defaultMessage: 'Open',
    },
    propertySnowy: {
        id: 'app.lab.schematic-preview.state.property.snowy',
        defaultMessage: 'Snowy',
    },
    propertyPersistent: {
        id: 'app.lab.schematic-preview.state.property.persistent',
        defaultMessage: 'Persistent',
    },
    propertyConditional: {
        id: 'app.lab.schematic-preview.state.property.conditional',
        defaultMessage: 'Conditional',
    },
    propertyDistance: {
        id: 'app.lab.schematic-preview.state.property.distance',
        defaultMessage: 'Distance',
    },
    propertyLevel: {
        id: 'app.lab.schematic-preview.state.property.level',
        defaultMessage: 'Level',
    },
    propertyPower: {
        id: 'app.lab.schematic-preview.state.property.power',
        defaultMessage: 'Power',
    },
    propertyAge: {
        id: 'app.lab.schematic-preview.state.property.age',
        defaultMessage: 'Age',
    },
    propertyStage: {
        id: 'app.lab.schematic-preview.state.property.stage',
        defaultMessage: 'Stage',
    },
    propertyNote: {
        id: 'app.lab.schematic-preview.state.property.note',
        defaultMessage: 'Note',
    },
    propertyInstrument: {
        id: 'app.lab.schematic-preview.state.property.instrument',
        defaultMessage: 'Instrument',
    },
    propertyDelay: {
        id: 'app.lab.schematic-preview.state.property.delay',
        defaultMessage: 'Delay',
    },
    propertyBites: {
        id: 'app.lab.schematic-preview.state.property.bites',
        defaultMessage: 'Bites',
    },
    propertyInWall: {
        id: 'app.lab.schematic-preview.state.property.in_wall',
        defaultMessage: 'In wall',
    },
    propertyAttached: {
        id: 'app.lab.schematic-preview.state.property.attached',
        defaultMessage: 'Attached',
    },
    propertyDisarmed: {
        id: 'app.lab.schematic-preview.state.property.disarmed',
        defaultMessage: 'Disarmed',
    },
    propertyExtended: {
        id: 'app.lab.schematic-preview.state.property.extended',
        defaultMessage: 'Extended',
    },
    propertyOccupied: {
        id: 'app.lab.schematic-preview.state.property.occupied',
        defaultMessage: 'Occupied',
    },
    propertyHanging: {
        id: 'app.lab.schematic-preview.state.property.hanging',
        defaultMessage: 'Hanging',
    },
    propertyLocked: {
        id: 'app.lab.schematic-preview.state.property.locked',
        defaultMessage: 'Locked',
    },
    propertySignalFire: {
        id: 'app.lab.schematic-preview.state.property.signal_fire',
        defaultMessage: 'Signal fire',
    },
    propertyHasRecord: {
        id: 'app.lab.schematic-preview.state.property.has_record',
        defaultMessage: 'Has record',
    },
    propertyHasBook: {
        id: 'app.lab.schematic-preview.state.property.has_book',
        defaultMessage: 'Has book',
    },
    propertyEye: {
        id: 'app.lab.schematic-preview.state.property.eye',
        defaultMessage: 'Eye',
    },
    propertyUp: {
        id: 'app.lab.schematic-preview.state.property.up',
        defaultMessage: 'Up',
    },
    propertyDown: {
        id: 'app.lab.schematic-preview.state.property.down',
        defaultMessage: 'Down',
    },
    propertyNorth: {
        id: 'app.lab.schematic-preview.state.property.north',
        defaultMessage: 'North',
    },
    propertyEast: {
        id: 'app.lab.schematic-preview.state.property.east',
        defaultMessage: 'East',
    },
    propertySouth: {
        id: 'app.lab.schematic-preview.state.property.south',
        defaultMessage: 'South',
    },
    propertyWest: {
        id: 'app.lab.schematic-preview.state.property.west',
        defaultMessage: 'West',
    },
    valueNorth: {
        id: 'app.lab.schematic-preview.state.value.north',
        defaultMessage: 'North',
    },
    valueEast: {
        id: 'app.lab.schematic-preview.state.value.east',
        defaultMessage: 'East',
    },
    valueSouth: {
        id: 'app.lab.schematic-preview.state.value.south',
        defaultMessage: 'South',
    },
    valueWest: {
        id: 'app.lab.schematic-preview.state.value.west',
        defaultMessage: 'West',
    },
    valueUp: {
        id: 'app.lab.schematic-preview.state.value.up',
        defaultMessage: 'Up',
    },
    valueDown: {
        id: 'app.lab.schematic-preview.state.value.down',
        defaultMessage: 'Down',
    },
    valueTop: {
        id: 'app.lab.schematic-preview.state.value.top',
        defaultMessage: 'Top',
    },
    valueBottom: {
        id: 'app.lab.schematic-preview.state.value.bottom',
        defaultMessage: 'Bottom',
    },
    valueUpper: {
        id: 'app.lab.schematic-preview.state.value.upper',
        defaultMessage: 'Upper',
    },
    valueLower: {
        id: 'app.lab.schematic-preview.state.value.lower',
        defaultMessage: 'Lower',
    },
    valueDouble: {
        id: 'app.lab.schematic-preview.state.value.double',
        defaultMessage: 'Double',
    },
    valueFloor: {
        id: 'app.lab.schematic-preview.state.value.floor',
        defaultMessage: 'Floor',
    },
    valueWall: {
        id: 'app.lab.schematic-preview.state.value.wall',
        defaultMessage: 'Wall',
    },
    valueCeiling: {
        id: 'app.lab.schematic-preview.state.value.ceiling',
        defaultMessage: 'Ceiling',
    },
    valueStraight: {
        id: 'app.lab.schematic-preview.state.value.straight',
        defaultMessage: 'Straight',
    },
    valueInnerLeft: {
        id: 'app.lab.schematic-preview.state.value.inner_left',
        defaultMessage: 'Inner left',
    },
    valueInnerRight: {
        id: 'app.lab.schematic-preview.state.value.inner_right',
        defaultMessage: 'Inner right',
    },
    valueOuterLeft: {
        id: 'app.lab.schematic-preview.state.value.outer_left',
        defaultMessage: 'Outer left',
    },
    valueOuterRight: {
        id: 'app.lab.schematic-preview.state.value.outer_right',
        defaultMessage: 'Outer right',
    },
    valueLeft: {
        id: 'app.lab.schematic-preview.state.value.left',
        defaultMessage: 'Left',
    },
    valueRight: {
        id: 'app.lab.schematic-preview.state.value.right',
        defaultMessage: 'Right',
    },
    valueNone: {
        id: 'app.lab.schematic-preview.state.value.none',
        defaultMessage: 'None',
    },
    valueSide: {
        id: 'app.lab.schematic-preview.state.value.side',
        defaultMessage: 'Side',
    },
    valueHead: {
        id: 'app.lab.schematic-preview.state.value.head',
        defaultMessage: 'Head',
    },
    valueFoot: {
        id: 'app.lab.schematic-preview.state.value.foot',
        defaultMessage: 'Foot',
    },
    valueSingle: {
        id: 'app.lab.schematic-preview.state.value.single',
        defaultMessage: 'Single',
    },
    valueSingleWall: {
        id: 'app.lab.schematic-preview.state.value.single_wall',
        defaultMessage: 'Single wall',
    },
    valueMiddleWall: {
        id: 'app.lab.schematic-preview.state.value.middle_wall',
        defaultMessage: 'Middle wall',
    },
    valueDoubleWall: {
        id: 'app.lab.schematic-preview.state.value.double_wall',
        defaultMessage: 'Double wall',
    },
    valueTip: {
        id: 'app.lab.schematic-preview.state.value.tip',
        defaultMessage: 'Tip',
    },
    valueMiddle: {
        id: 'app.lab.schematic-preview.state.value.middle',
        defaultMessage: 'Middle',
    },
    valueBase: {
        id: 'app.lab.schematic-preview.state.value.base',
        defaultMessage: 'Base',
    },
    valueFrustum: {
        id: 'app.lab.schematic-preview.state.value.frustum',
        defaultMessage: 'Frustum',
    },
    valuePartial: {
        id: 'app.lab.schematic-preview.state.value.partial',
        defaultMessage: 'Partial',
    },
    valueUnstable: {
        id: 'app.lab.schematic-preview.state.value.unstable',
        defaultMessage: 'Unstable',
    },
    valueFull: {
        id: 'app.lab.schematic-preview.state.value.full',
        defaultMessage: 'Full',
    },
})

const PROPERTY_MESSAGES: Record<string, keyof typeof messages> = {
    facing: 'propertyFacing',
    axis: 'propertyAxis',
    half: 'propertyHalf',
    type: 'propertyType',
    shape: 'propertyShape',
    face: 'propertyFace',
    attachment: 'propertyAttachment',
    hinge: 'propertyHinge',
    part: 'propertyPart',
    orientation: 'propertyOrientation',
    rotation: 'propertyRotation',
    vertical_direction: 'propertyVerticalDirection',
    tilt: 'propertyTilt',
    waterlogged: 'propertyWaterlogged',
    powered: 'propertyPowered',
    lit: 'propertyLit',
    open: 'propertyOpen',
    snowy: 'propertySnowy',
    persistent: 'propertyPersistent',
    conditional: 'propertyConditional',
    distance: 'propertyDistance',
    level: 'propertyLevel',
    power: 'propertyPower',
    age: 'propertyAge',
    stage: 'propertyStage',
    note: 'propertyNote',
    instrument: 'propertyInstrument',
    delay: 'propertyDelay',
    bites: 'propertyBites',
    in_wall: 'propertyInWall',
    attached: 'propertyAttached',
    disarmed: 'propertyDisarmed',
    extended: 'propertyExtended',
    occupied: 'propertyOccupied',
    hanging: 'propertyHanging',
    locked: 'propertyLocked',
    signal_fire: 'propertySignalFire',
    has_record: 'propertyHasRecord',
    has_book: 'propertyHasBook',
    eye: 'propertyEye',
    up: 'propertyUp',
    down: 'propertyDown',
    north: 'propertyNorth',
    east: 'propertyEast',
    south: 'propertySouth',
    west: 'propertyWest',
}

const VALUE_MESSAGES: Record<string, keyof typeof messages> = {
    north: 'valueNorth',
    east: 'valueEast',
    south: 'valueSouth',
    west: 'valueWest',
    up: 'valueUp',
    down: 'valueDown',
    top: 'valueTop',
    bottom: 'valueBottom',
    upper: 'valueUpper',
    lower: 'valueLower',
    double: 'valueDouble',
    floor: 'valueFloor',
    wall: 'valueWall',
    ceiling: 'valueCeiling',
    straight: 'valueStraight',
    inner_left: 'valueInnerLeft',
    inner_right: 'valueInnerRight',
    outer_left: 'valueOuterLeft',
    outer_right: 'valueOuterRight',
    left: 'valueLeft',
    right: 'valueRight',
    none: 'valueNone',
    side: 'valueSide',
    head: 'valueHead',
    foot: 'valueFoot',
    single: 'valueSingle',
    single_wall: 'valueSingleWall',
    middle_wall: 'valueMiddleWall',
    double_wall: 'valueDoubleWall',
    tip: 'valueTip',
    middle: 'valueMiddle',
    base: 'valueBase',
    frustum: 'valueFrustum',
    partial: 'valuePartial',
    unstable: 'valueUnstable',
    full: 'valueFull',
}

const CHIP_LIMIT = 8

const hasReadOnly = computed(() => props.properties.some((property) => property.values.length <= 1))

function propertyLabel(key: string) {
    const message = PROPERTY_MESSAGES[key]
    return message ? formatMessage(messages[message]) : key
}

function valueLabel(value: string) {
    const message = VALUE_MESSAGES[value]
    return message ? formatMessage(messages[message]) : value
}

function chipClass(active: boolean) {
    return [
        'cursor-pointer rounded-[var(--radius-sm)] border border-solid px-2 py-1 text-base transition-colors',
        active
            ? 'border-brand bg-brand-highlight text-brand'
            : 'border-surface-5 bg-surface-3 text-[var(--color-text-primary)] hover:bg-surface-4',
        'disabled:cursor-not-allowed disabled:opacity-50',
    ]
}

function selectProperty(key: string, event: Event) {
    emit('change', key, (event.target as HTMLSelectElement).value)
}
</script>

<template>
    <div class="flex flex-col gap-2">
        <p v-if="properties.length === 0" class="m-0 text-base text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.empty) }}
        </p>
        <div
            v-for="property in properties"
            :key="property.key"
            class="grid grid-cols-[minmax(0,5rem)_minmax(0,1fr)] items-center gap-2"
        >
            <span
                v-tooltip="property.key"
                class="truncate text-base text-[var(--color-text-tertiary)]"
                >{{ propertyLabel(property.key) }}</span
            >
            <Toggle
                v-if="property.boolean"
                class="justify-self-start"
                :model-value="property.value === 'true'"
                :disabled="disabled"
                :aria-label="propertyLabel(property.key)"
                @update:model-value="(value) => emit('change', property.key, String(value))"
            />
            <div
                v-else-if="property.values.length <= CHIP_LIMIT"
                class="flex flex-wrap gap-1"
                role="group"
                :aria-label="propertyLabel(property.key)"
            >
                <button
                    v-for="value in property.values"
                    :key="value"
                    v-tooltip="value"
                    type="button"
                    :class="chipClass(value === property.value)"
                    :aria-pressed="value === property.value"
                    :disabled="disabled || property.values.length <= 1"
                    @click="emit('change', property.key, value)"
                >
                    {{ valueLabel(value) }}
                </button>
            </div>
            <select
                v-else
                class="w-full cursor-pointer rounded-[var(--radius-sm)] border border-surface-5 bg-surface-3 px-2 py-1 text-base text-[var(--color-text-primary)] disabled:cursor-not-allowed disabled:opacity-50"
                :value="property.value"
                :disabled="disabled"
                :aria-label="propertyLabel(property.key)"
                @change="(event) => selectProperty(property.key, event)"
            >
                <option v-for="value in property.values" :key="value" :value="value">
                    {{ valueLabel(value) }}
                </option>
            </select>
        </div>
        <p v-if="hasReadOnly" class="m-0 text-base text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.readonly) }}
        </p>
    </div>
</template>
