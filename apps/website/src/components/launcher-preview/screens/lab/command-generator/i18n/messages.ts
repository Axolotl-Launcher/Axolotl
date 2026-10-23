import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/lab-command-generator.zh-CN'

import { registerPreviewMessages } from '../../../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

export const commandGeneratorMessages = defineMessages({
    v26ArgWaypointActionDisplay: {
        id: 'app.lab.command-generator.arg.waypoint.action.display',
        defaultMessage: 'Action',
    },
    v26ArgWaypointActionDesc: {
        id: 'app.lab.command-generator.arg.waypoint.action.desc',
        defaultMessage: 'Action',
    },
    v26ArgWaypointWaypointDisplay: {
        id: 'app.lab.command-generator.arg.waypoint.waypoint.display',
        defaultMessage: 'Waypoint entity',
    },
    v26ArgWaypointWaypointDesc: {
        id: 'app.lab.command-generator.arg.waypoint.waypoint.desc',
        defaultMessage: 'Waypoint entity',
    },
    v26ArgWaypointPropertyDisplay: {
        id: 'app.lab.command-generator.arg.waypoint.property.display',
        defaultMessage: 'Property',
    },
    v26ArgWaypointPropertyDesc: {
        id: 'app.lab.command-generator.arg.waypoint.property.desc',
        defaultMessage: 'Property',
    },
    v26ArgWaypointValueDisplay: {
        id: 'app.lab.command-generator.arg.waypoint.value.display',
        defaultMessage: 'Color or style',
    },
    v26ArgWaypointValueDesc: {
        id: 'app.lab.command-generator.arg.waypoint.value.desc',
        defaultMessage: 'Color or style',
    },
    rangeError: {
        id: 'app.lab.command-generator.error.range',
        defaultMessage: 'Enter a value within the allowed range for {name}.',
    },
    v26ArgTimeClockDisplay: {
        id: 'app.lab.command-generator.arg.time.clock.display',
        defaultMessage: 'World clock',
    },
    v26ArgTimeClockDesc: {
        id: 'app.lab.command-generator.arg.time.clock.desc',
        defaultMessage: 'Leave empty to use the dimension’s default clock.',
    },
    v26ArgTimeRateDisplay: {
        id: 'app.lab.command-generator.arg.time.rate.display',
        defaultMessage: 'Clock rate',
    },
    v26ArgTimeRateDesc: {
        id: 'app.lab.command-generator.arg.time.rate.desc',
        defaultMessage: 'A multiplier greater than 0 and at most 1000.',
    },
    v26PresetTimeClockTime: {
        id: 'app.lab.command-generator.preset.time.clock-time',
        defaultMessage: 'Clock ticks',
    },
    v26PresetTimeGameTime: {
        id: 'app.lab.command-generator.preset.time.game-time',
        defaultMessage: 'Elapsed game ticks',
    },
    v26PresetTimeDayCycles: {
        id: 'app.lab.command-generator.preset.time.day-cycles',
        defaultMessage: 'Day cycles',
    },
    v26ArgTimeQueryDisplay: {
        id: 'app.lab.command-generator.arg.time.query.display',
        defaultMessage: 'Query',
    },
    v26ArgTimeQueryDesc: {
        id: 'app.lab.command-generator.arg.time.query.desc',
        defaultMessage: 'Use time, gametime, or a timeline ID; append repetition to count cycles.',
    },
    v26PresetGameruleFireRadius: {
        id: 'app.lab.command-generator.preset.gamerule.fire-radius',
        defaultMessage: 'Fire spread radius',
    },
    v26ArgGameruleValueDesc26: {
        id: 'app.lab.command-generator.arg.gamerule.value.desc.26',
        defaultMessage:
            'A boolean or integer, depending on the game rule. Fire spread radius 0 disables spread.',
    },
    v26ArgDialogActionDisplay: {
        id: 'app.lab.command-generator.arg.dialog.action.display',
        defaultMessage: 'Action',
    },
    v26ArgDialogActionDesc: {
        id: 'app.lab.command-generator.arg.dialog.action.desc',
        defaultMessage: 'Show or clear a dialog.',
    },
    v26ArgFetchprofileSourceDisplay: {
        id: 'app.lab.command-generator.arg.fetchprofile.source.display',
        defaultMessage: 'Profile source',
    },
    v26ArgFetchprofileSourceDesc: {
        id: 'app.lab.command-generator.arg.fetchprofile.source.desc',
        defaultMessage: 'Fetch by player name, UUID, or entity.',
    },
    v26ArgFetchprofileProfileDisplay: {
        id: 'app.lab.command-generator.arg.fetchprofile.profile.display',
        defaultMessage: 'Name, UUID or selector',
    },
    v26ArgFetchprofileProfileDesc: {
        id: 'app.lab.command-generator.arg.fetchprofile.profile.desc',
        defaultMessage: 'Enter a player name, UUID, or a single entity selector.',
    },
    v26CmdFetchprofileDesc: {
        id: 'app.lab.command-generator.cmd.fetchprofile.desc',
        defaultMessage: 'Fetch a player profile.',
    },
    v26CmdUnpublishDesc: {
        id: 'app.lab.command-generator.cmd.unpublish.desc',
        defaultMessage: 'Stop sharing the single-player world.',
    },
    v26ArgComputeContextDisplay: {
        id: 'app.lab.command-generator.arg.compute.context.display',
        defaultMessage: 'Context',
    },
    v26ArgComputeContextDesc: {
        id: 'app.lab.command-generator.arg.compute.context.desc',
        defaultMessage: 'Choose the context in which to evaluate the provider.',
    },
    v26ArgComputePosDisplay: {
        id: 'app.lab.command-generator.arg.compute.pos.display',
        defaultMessage: 'Block position',
    },
    v26ArgComputePosDesc: {
        id: 'app.lab.command-generator.arg.compute.pos.desc',
        defaultMessage: 'Block position',
    },
    v26ArgComputeTargetDisplay: {
        id: 'app.lab.command-generator.arg.compute.target.display',
        defaultMessage: 'Entity',
    },
    v26ArgComputeTargetDesc: {
        id: 'app.lab.command-generator.arg.compute.target.desc',
        defaultMessage: 'Select one entity.',
    },
    v26ArgComputeTypeDisplay: {
        id: 'app.lab.command-generator.arg.compute.type.display',
        defaultMessage: 'Result type',
    },
    v26ArgComputeTypeDesc: {
        id: 'app.lab.command-generator.arg.compute.type.desc',
        defaultMessage: 'Result type',
    },
    v26ArgComputeProviderDisplay: {
        id: 'app.lab.command-generator.arg.compute.provider.display',
        defaultMessage: 'Number provider',
    },
    v26ArgComputeProviderDesc: {
        id: 'app.lab.command-generator.arg.compute.provider.desc',
        defaultMessage: 'Enter an inline number provider or a namespaced registry ID.',
    },
    v26ArgComputeScaleDisplay: {
        id: 'app.lab.command-generator.arg.compute.scale.display',
        defaultMessage: 'Scale',
    },
    v26ArgComputeScaleDesc: {
        id: 'app.lab.command-generator.arg.compute.scale.desc',
        defaultMessage: 'Scale the float result before returning an integer.',
    },
    v26CmdComputeDesc: {
        id: 'app.lab.command-generator.cmd.compute.desc',
        defaultMessage: 'Evaluate a number provider.',
    },
    v26ArgPosteffectActionDisplay: {
        id: 'app.lab.command-generator.arg.posteffect.action.display',
        defaultMessage: 'Action',
    },
    v26ArgPosteffectActionDesc: {
        id: 'app.lab.command-generator.arg.posteffect.action.desc',
        defaultMessage: 'Action',
    },
    v26ArgPosteffectTargetDisplay: {
        id: 'app.lab.command-generator.arg.posteffect.target.display',
        defaultMessage: 'Players',
    },
    v26ArgPosteffectTargetDesc: {
        id: 'app.lab.command-generator.arg.posteffect.target.desc',
        defaultMessage: 'Players',
    },
    v26ArgPosteffectEffectDisplay: {
        id: 'app.lab.command-generator.arg.posteffect.effect.display',
        defaultMessage: 'Post effect',
    },
    v26ArgPosteffectEffectDesc: {
        id: 'app.lab.command-generator.arg.posteffect.effect.desc',
        defaultMessage: 'Resource ID of a client-side postprocessing effect.',
    },
    v26CmdPosteffectDesc: {
        id: 'app.lab.command-generator.cmd.posteffect.desc',
        defaultMessage: 'Manage screen postprocessing effects.',
    },
    v26ArgSwingAnimationDisplay: {
        id: 'app.lab.command-generator.arg.swing.animation.display',
        defaultMessage: 'Animation',
    },
    v26ArgSwingAnimationDesc: {
        id: 'app.lab.command-generator.arg.swing.animation.desc',
        defaultMessage: 'Animation',
    },
    v26ArgSwingDurationDisplay: {
        id: 'app.lab.command-generator.arg.swing.duration.display',
        defaultMessage: 'Animation duration',
    },
    v26ArgSwingDurationDesc: {
        id: 'app.lab.command-generator.arg.swing.duration.desc',
        defaultMessage: 'Positive duration in ticks, seconds (s), or days (d).',
    },
    v26ArgItemSlotDesc26: {
        id: 'app.lab.command-generator.arg.item.slot.desc.26',
        defaultMessage: 'Slot range, inline slot source, or namespaced slot-source ID.',
    },
    v26ArgDataMethodDisplay: {
        id: 'app.lab.command-generator.arg.data.method.display',
        defaultMessage: 'Modification',
    },
    v26ArgDataMethodDesc: {
        id: 'app.lab.command-generator.arg.data.method.desc',
        defaultMessage: 'Modification',
    },
    v26ArgDataSourceDisplay: {
        id: 'app.lab.command-generator.arg.data.source.display',
        defaultMessage: 'Value source',
    },
    v26ArgDataSourceDesc: {
        id: 'app.lab.command-generator.arg.data.source.desc',
        defaultMessage: 'Value source',
    },
    v26ArgDataIndexDisplay: {
        id: 'app.lab.command-generator.arg.data.index.display',
        defaultMessage: 'Insert index',
    },
    v26ArgDataIndexDesc: {
        id: 'app.lab.command-generator.arg.data.index.desc',
        defaultMessage: 'Insert index',
    },
    v26ArgDataLiteralValueDisplay: {
        id: 'app.lab.command-generator.arg.data.literalValue.display',
        defaultMessage: 'SNBT value',
    },
    v26ArgDataLiteralValueDesc: {
        id: 'app.lab.command-generator.arg.data.literalValue.desc',
        defaultMessage: 'SNBT value',
    },
    v26ExecuteIfSlotsLabel: {
        id: 'app.lab.command-generator.execute.if-slots.label',
        defaultMessage: 'if slots',
    },
    v26ExecuteIfSlotsDesc: {
        id: 'app.lab.command-generator.execute.if-slots.desc',
        defaultMessage: 'Test slots on a block or entity using a slot source.',
    },
    v26ArgExecuteSourceDisplay: {
        id: 'app.lab.command-generator.arg.execute.source.display',
        defaultMessage: 'Source type',
    },
    v26ArgExecuteSourceDesc: {
        id: 'app.lab.command-generator.arg.execute.source.desc',
        defaultMessage: 'Source type',
    },
    v26ArgExecuteTargetDisplay: {
        id: 'app.lab.command-generator.arg.execute.target.display',
        defaultMessage: 'Entity or position',
    },
    v26ArgExecuteTargetDesc: {
        id: 'app.lab.command-generator.arg.execute.target.desc',
        defaultMessage: 'Entity or position',
    },
    v26ArgExecuteSlotsDisplay: {
        id: 'app.lab.command-generator.arg.execute.slots.display',
        defaultMessage: 'Slot source',
    },
    v26ArgExecuteSlotsDesc: {
        id: 'app.lab.command-generator.arg.execute.slots.desc',
        defaultMessage: 'A slot range, inline slot source or namespaced ID.',
    },
    v26ExecuteUnlessSlotsLabel: {
        id: 'app.lab.command-generator.execute.unless-slots.label',
        defaultMessage: 'unless slots',
    },
    v26ExecuteUnlessSlotsDesc: {
        id: 'app.lab.command-generator.execute.unless-slots.desc',
        defaultMessage: 'Test slots on a block or entity using a slot source.',
    },
    v26ArgExecuteSlotDesc26: {
        id: 'app.lab.command-generator.arg.execute.slot.desc.26',
        defaultMessage: 'Slot range or slot source.',
    },
    copyFailed: {
        id: 'app.lab.command-generator.copy-failed',
        defaultMessage: 'Could not copy the command. Select and copy the text manually.',
    },
    requiredError: {
        id: 'app.lab.command-generator.error.required',
        defaultMessage: 'Enter {name}.',
    },
    enumError: {
        id: 'app.lab.command-generator.error.enum',
        defaultMessage: 'Choose a valid value for {name}.',
    },
    numberError: {
        id: 'app.lab.command-generator.error.number',
        defaultMessage: 'Enter a number for {name}.',
    },
    integerError: {
        id: 'app.lab.command-generator.error.integer',
        defaultMessage: 'Enter a whole number for {name}.',
    },
    filters: { id: 'app.lab.command-generator.filters', defaultMessage: 'Filters' },
    close: { id: 'app.lab.command-generator.close', defaultMessage: 'Close' },
    save: { id: 'app.lab.command-generator.save', defaultMessage: 'Save' },
    deletePreset: {
        id: 'app.lab.command-generator.preset.delete',
        defaultMessage: 'Delete preset',
    },
    startTitle: {
        id: 'app.lab.command-generator.start-title',
        defaultMessage: 'Choose a command to get started',
    },
    startHint: {
        id: 'app.lab.command-generator.start-hint',
        defaultMessage: 'Search by command name, or start with a common command.',
    },
    resultHint: {
        id: 'app.lab.command-generator.result-hint',
        defaultMessage: 'Fill in the parameters, then generate a command to copy into Minecraft.',
    },
    optional: { id: 'app.lab.command-generator.arg.optional', defaultMessage: 'Optional' },
    moveUp: { id: 'app.lab.command-generator.execute.move-up', defaultMessage: 'Move up' },
    moveDown: { id: 'app.lab.command-generator.execute.move-down', defaultMessage: 'Move down' },
    removeStep: {
        id: 'app.lab.command-generator.execute.remove-step',
        defaultMessage: 'Remove subcommand',
    },
    removeLore: {
        id: 'app.lab.command-generator.item-nbt.remove-lore',
        defaultMessage: 'Remove lore line',
    },
    removeEnchantment: {
        id: 'app.lab.command-generator.item-nbt.remove-enchantment',
        defaultMessage: 'Remove enchantment',
    },
    tagName: { id: 'app.lab.command-generator.nbt.tag-name', defaultMessage: 'Tag name' },
    tagValue: { id: 'app.lab.command-generator.nbt.tag-value', defaultMessage: 'Tag value' },
    toggleTag: {
        id: 'app.lab.command-generator.nbt.toggle-tag',
        defaultMessage: 'Expand or collapse tag',
    },
    addTag: { id: 'app.lab.command-generator.nbt.add-tag', defaultMessage: 'Add child tag' },
    removeTag: { id: 'app.lab.command-generator.nbt.remove-tag', defaultMessage: 'Remove tag' },
    itemTargetType: {
        id: 'app.lab.command-generator.arg.item.targetType.display',
        defaultMessage: 'Target type',
    },
    itemTargetTypeDescription: {
        id: 'app.lab.command-generator.arg.item.targetType.desc',
        defaultMessage: 'Choose a block or an entity.',
    },
    itemTarget: {
        id: 'app.lab.command-generator.arg.item.target.display',
        defaultMessage: 'Target entity',
    },
    itemTargetDescription: {
        id: 'app.lab.command-generator.arg.item.target.desc',
        defaultMessage: 'Entity whose item will be changed.',
    },
    itemSource: {
        id: 'app.lab.command-generator.arg.item.source.display',
        defaultMessage: 'Replacement source',
    },
    itemSourceDescription: {
        id: 'app.lab.command-generator.arg.item.source.desc',
        defaultMessage: 'Replace the slot with the selected item.',
    },
    itemModifier: {
        id: 'app.lab.command-generator.arg.item.modifier.display',
        defaultMessage: 'Item modifier',
    },
    itemModifierDescription: {
        id: 'app.lab.command-generator.arg.item.modifier.desc',
        defaultMessage: 'Resource ID of the item modifier.',
    },
    advancementMode: {
        id: 'app.lab.command-generator.arg.advancement.mode.display',
        defaultMessage: 'Selection mode',
    },
    advancementModeDescription: {
        id: 'app.lab.command-generator.arg.advancement.mode.desc',
        defaultMessage: 'Choose one advancement, a tree, or everything.',
    },
    locateBiomeDescription: {
        id: 'app.lab.command-generator.cmd.locatebiome.desc',
        defaultMessage: 'Locate the nearest biome.',
    },
    // ===== 页面标题 =====
    title: { id: 'app.lab.command-generator.title', defaultMessage: 'Command generator' },
    subtitle: {
        id: 'app.lab.command-generator.subtitle',
        defaultMessage: 'Build Minecraft Java Edition commands visually.',
    },
    selectVersion: {
        id: 'app.lab.command-generator.select-version',
        defaultMessage: 'Game version',
    },
    selectCommand: {
        id: 'app.lab.command-generator.select-command',
        defaultMessage: 'Choose a command',
    },
    selectPlaceholder: {
        id: 'app.lab.command-generator.select-placeholder',
        defaultMessage: 'Search a command…',
    },
    parametersTitle: {
        id: 'app.lab.command-generator.parameters-title',
        defaultMessage: 'Parameters',
    },
    parametersEmpty: {
        id: 'app.lab.command-generator.parameters-empty',
        defaultMessage: 'This command takes no arguments.',
    },
    generate: { id: 'app.lab.command-generator.generate', defaultMessage: 'Generate' },
    copy: { id: 'app.lab.command-generator.copy', defaultMessage: 'Copy' },
    copied: { id: 'app.lab.command-generator.copied', defaultMessage: 'Copied' },
    result: { id: 'app.lab.command-generator.result', defaultMessage: 'Generated command' },

    // ===== 筛选器 =====
    filterCategory: { id: 'app.lab.command-generator.filter.category', defaultMessage: 'Category' },
    filterPermission: {
        id: 'app.lab.command-generator.filter.permission',
        defaultMessage: 'Permission level',
    },
    filterAll: { id: 'app.lab.command-generator.filter.all', defaultMessage: 'All' },

    // ===== 预设 =====
    savePreset: { id: 'app.lab.command-generator.preset.save', defaultMessage: 'Save as preset' },
    presetName: { id: 'app.lab.command-generator.preset.name', defaultMessage: 'Preset name' },
    myPresets: { id: 'app.lab.command-generator.preset.my', defaultMessage: 'My presets' },
    globalPresets: { id: 'app.lab.command-generator.preset.global', defaultMessage: 'All presets' },
    globalPresetsEmpty: {
        id: 'app.lab.command-generator.preset.global-empty',
        defaultMessage: 'No saved presets yet.',
    },
    globalPresetsHint: {
        id: 'app.lab.command-generator.preset.global-hint',
        defaultMessage: 'Click a preset to jump to its version and command.',
    },
    toastPresetSaved: {
        id: 'app.lab.command-generator.preset.toast.saved',
        defaultMessage: 'Preset "{name}" saved',
    },
    toastPresetLoaded: {
        id: 'app.lab.command-generator.preset.toast.loaded',
        defaultMessage: 'Preset "{name}" loaded',
    },
    toastPresetDeleted: {
        id: 'app.lab.command-generator.preset.toast.deleted',
        defaultMessage: 'Preset "{name}" deleted',
    },
    toastPresetNameRequired: {
        id: 'app.lab.command-generator.preset.toast.name-required',
        defaultMessage: 'Please enter a preset name',
    },
    toastPresetCommandMissing: {
        id: 'app.lab.command-generator.preset.toast.command-missing',
        defaultMessage:
            'Preset "{name}" uses command "{command}" which does not exist in this version',
    },

    // ===== 参数卡片 =====
    argSelectPlaceholder: {
        id: 'app.lab.command-generator.arg.select-placeholder',
        defaultMessage: 'Select…',
    },
    argRequired: { id: 'app.lab.command-generator.arg.required', defaultMessage: 'Required' },

    // ===== Execute 链式构建器 =====
    executeChainTitle: {
        id: 'app.lab.command-generator.execute.chain-title',
        defaultMessage: 'Subcommand chain',
    },
    executeChainEmpty: {
        id: 'app.lab.command-generator.execute.chain-empty',
        defaultMessage: 'Click a subcommand below to add it to the chain.',
    },
    executePickerTitle: {
        id: 'app.lab.command-generator.execute.picker-title',
        defaultMessage: 'Available subcommands',
    },
    executeFinalTitle: {
        id: 'app.lab.command-generator.execute.final-title',
        defaultMessage: 'Command to run',
    },
    executeFinalPlaceholder: {
        id: 'app.lab.command-generator.execute.final-placeholder',
        defaultMessage: 'say hello',
    },
    executeFinalHint: {
        id: 'app.lab.command-generator.execute.final-hint',
        defaultMessage: 'Type the actual command without the leading slash.',
    },
    executeQuickExamples: {
        id: 'app.lab.command-generator.execute.quick-examples',
        defaultMessage: 'Quick examples',
    },
    executeCommonCommands: {
        id: 'app.lab.command-generator.execute.common-commands',
        defaultMessage: 'Common commands',
    },
    executeClear: { id: 'app.lab.command-generator.execute.clear', defaultMessage: 'Clear' },
    executeBuildFirst: {
        id: 'app.lab.command-generator.execute.build-first',
        defaultMessage: 'Please build the subcommand chain first',
    },

    // ===== 快速预设（execute 内置示例） =====
    quickPresetAllSayHello: {
        id: 'app.lab.command-generator.execute.quick.all-say-hello',
        defaultMessage: 'All players say hello',
    },
    quickPresetCheckZombies: {
        id: 'app.lab.command-generator.execute.quick.check-zombies',
        defaultMessage: 'Check for zombies',
    },
    quickPresetKillNearbyZombies: {
        id: 'app.lab.command-generator.execute.quick.kill-nearby-zombies',
        defaultMessage: 'Kill nearby zombies and count',
    },
    quickPresetGrassBelow: {
        id: 'app.lab.command-generator.execute.quick.grass-below',
        defaultMessage: 'Say when grass is below',
    },
    quickPresetMoveForward: {
        id: 'app.lab.command-generator.execute.quick.move-forward',
        defaultMessage: 'All entities move forward',
    },

    // ===== NBT 编辑器 =====
    nbtTitle: { id: 'app.lab.command-generator.nbt.title', defaultMessage: 'NBT editor' },
    nbtSimple: { id: 'app.lab.command-generator.nbt.simple', defaultMessage: 'Simple' },
    nbtAdvanced: { id: 'app.lab.command-generator.nbt.advanced', defaultMessage: 'Advanced' },
    nbtAdvancedTitle: {
        id: 'app.lab.command-generator.nbt.advanced-title',
        defaultMessage: 'Advanced NBT tree',
    },
    nbtAddRoot: {
        id: 'app.lab.command-generator.nbt.add-root',
        defaultMessage: 'Add root tag',
    },
    nbtSnbtPreview: {
        id: 'app.lab.command-generator.nbt.snbt-preview',
        defaultMessage: 'SNBT preview',
    },
    nbtPresetEnchanted: {
        id: 'app.lab.command-generator.nbt.preset-enchanted',
        defaultMessage: 'Enchanted item',
    },
    nbtPresetCustomName: {
        id: 'app.lab.command-generator.nbt.preset-custom-name',
        defaultMessage: 'Custom name',
    },

    // ===== 物品 NBT 傻瓜编辑器 =====
    itemNbtTitle: {
        id: 'app.lab.command-generator.item-nbt.title',
        defaultMessage: 'Item customisation',
    },
    itemNbtName: {
        id: 'app.lab.command-generator.item-nbt.name',
        defaultMessage: 'Custom name',
    },
    itemNbtNameHint: {
        id: 'app.lab.command-generator.item-nbt.name-hint',
        defaultMessage: 'Shown as the item title.',
    },
    itemNbtNamePlaceholder: {
        id: 'app.lab.command-generator.item-nbt.name-placeholder',
        defaultMessage: 'Legendary sword',
    },
    itemNbtLore: {
        id: 'app.lab.command-generator.item-nbt.lore',
        defaultMessage: 'Lore lines',
    },
    itemNbtLoreHint: {
        id: 'app.lab.command-generator.item-nbt.lore-hint',
        defaultMessage: 'Each line appears under the item name.',
    },
    itemNbtLorePlaceholder: {
        id: 'app.lab.command-generator.item-nbt.lore-placeholder',
        defaultMessage: 'A line of lore…',
    },
    itemNbtAddLore: {
        id: 'app.lab.command-generator.item-nbt.add-lore',
        defaultMessage: 'Add line',
    },
    itemNbtEnchantments: {
        id: 'app.lab.command-generator.item-nbt.enchantments',
        defaultMessage: 'Enchantments',
    },
    itemNbtEnchantmentsHint: {
        id: 'app.lab.command-generator.item-nbt.enchantments-hint',
        defaultMessage: 'Pick an enchantment and set its level.',
    },
    itemNbtAddEnchantment: {
        id: 'app.lab.command-generator.item-nbt.add-enchantment',
        defaultMessage: 'Add enchantment',
    },
    itemNbtLevel: { id: 'app.lab.command-generator.item-nbt.level', defaultMessage: 'Level' },
    itemNbtNoEnchantments: {
        id: 'app.lab.command-generator.item-nbt.no-enchantments',
        defaultMessage: 'No enchantments yet.',
    },
    itemNbtUnbreakable: {
        id: 'app.lab.command-generator.item-nbt.unbreakable',
        defaultMessage: 'Unbreakable',
    },
    itemNbtUnbreakableHint: {
        id: 'app.lab.command-generator.item-nbt.unbreakable-hint',
        defaultMessage: 'Prevents the item from taking durability damage.',
    },
    itemNbtPreview: {
        id: 'app.lab.command-generator.item-nbt.preview',
        defaultMessage: 'NBT preview',
    },
    itemNbtPreviewEmpty: {
        id: 'app.lab.command-generator.item-nbt.preview-empty',
        defaultMessage: 'Empty — no NBT will be applied.',
    },
})
