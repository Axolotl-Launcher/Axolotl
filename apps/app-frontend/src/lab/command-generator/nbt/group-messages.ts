/**
 * 结构化组的列名。
 *
 * 单独成文件是因为 `formatjs extract` 只认 `defineMessages` 里的静态 ID：
 * 把列名写成模板字符串拼出来的键，提取时会被当成「不存在的键」删掉。
 */

import { defineMessages } from '@modrinth/ui'

export const groupColumnMessages = defineMessages({
    enchantment: {
        id: 'app.lab.command-generator.nbt.group.column.enchantment',
        defaultMessage: 'Enchantment',
    },
    level: {
        id: 'app.lab.command-generator.nbt.group.column.level',
        defaultMessage: 'Level',
    },
    effect: {
        id: 'app.lab.command-generator.nbt.group.column.effect',
        defaultMessage: 'Effect',
    },
    amplifier: {
        id: 'app.lab.command-generator.nbt.group.column.amplifier',
        defaultMessage: 'Amplifier',
    },
    duration: {
        id: 'app.lab.command-generator.nbt.group.column.duration',
        defaultMessage: 'Duration (ticks)',
    },
    ambient: {
        id: 'app.lab.command-generator.nbt.group.column.ambient',
        defaultMessage: 'Beacon particles',
    },
    showParticles: {
        id: 'app.lab.command-generator.nbt.group.column.show-particles',
        defaultMessage: 'Show particles',
    },
    showIcon: {
        id: 'app.lab.command-generator.nbt.group.column.show-icon',
        defaultMessage: 'Show icon',
    },
    text: {
        id: 'app.lab.command-generator.nbt.group.column.text',
        defaultMessage: 'A line of text',
    },
    slot: {
        id: 'app.lab.command-generator.nbt.group.column.slot',
        defaultMessage: 'Slot',
    },
    item: {
        id: 'app.lab.command-generator.nbt.group.column.item',
        defaultMessage: 'Item',
    },
    count: {
        id: 'app.lab.command-generator.nbt.group.column.count',
        defaultMessage: 'Count',
    },
    attribute: {
        id: 'app.lab.command-generator.nbt.group.column.attribute',
        defaultMessage: 'Attribute',
    },
    amount: {
        id: 'app.lab.command-generator.nbt.group.column.amount',
        defaultMessage: 'Amount',
    },
    operation: {
        id: 'app.lab.command-generator.nbt.group.column.operation',
        defaultMessage: 'Operation',
    },
    attributeSlot: {
        id: 'app.lab.command-generator.nbt.group.column.attribute-slot',
        defaultMessage: 'Applies to slot',
    },
    modifierId: {
        id: 'app.lab.command-generator.nbt.group.column.modifier-id',
        defaultMessage: 'ID (optional)',
    },
    flightDuration: {
        id: 'app.lab.command-generator.nbt.group.column.flight-duration',
        defaultMessage: 'Flight duration',
    },
    shape: {
        id: 'app.lab.command-generator.nbt.group.column.shape',
        defaultMessage: 'Shape',
    },
    colors: {
        id: 'app.lab.command-generator.nbt.group.column.colors',
        defaultMessage: 'Colors',
    },
    fadeColors: {
        id: 'app.lab.command-generator.nbt.group.column.fade-colors',
        defaultMessage: 'Fade colors',
    },
    hasTrail: {
        id: 'app.lab.command-generator.nbt.group.column.has-trail',
        defaultMessage: 'Trail',
    },
    hasTwinkle: {
        id: 'app.lab.command-generator.nbt.group.column.has-twinkle',
        defaultMessage: 'Twinkle',
    },
    potion: {
        id: 'app.lab.command-generator.nbt.group.column.potion',
        defaultMessage: 'Potion',
    },
    customColor: {
        id: 'app.lab.command-generator.nbt.group.column.custom-color',
        defaultMessage: 'Custom color',
    },
})
