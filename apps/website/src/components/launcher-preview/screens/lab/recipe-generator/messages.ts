import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/lab-recipe.zh-CN'

import { registerPreviewMessages } from '../../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

export const recipeTypeMessages = defineMessages({
    brewing: { id: 'app.lab.recipe-generator.type.brewing', defaultMessage: 'Brewing' },
    crafting: { id: 'app.lab.recipe-generator.type.crafting', defaultMessage: 'Crafting' },
    smelting: { id: 'app.lab.recipe-generator.type.smelting', defaultMessage: 'Smelting' },
    blasting: { id: 'app.lab.recipe-generator.type.blasting', defaultMessage: 'Blasting' },
    smoking: { id: 'app.lab.recipe-generator.type.smoking', defaultMessage: 'Smoking' },
    campfire_cooking: {
        id: 'app.lab.recipe-generator.type.campfire-cooking',
        defaultMessage: 'Campfire Cooking',
    },
    stonecutter: { id: 'app.lab.recipe-generator.type.stonecutter', defaultMessage: 'Stonecutter' },
    smithing: { id: 'app.lab.recipe-generator.type.smithing', defaultMessage: 'Smithing' },
    smithing_trim: {
        id: 'app.lab.recipe-generator.type.smithing-trim',
        defaultMessage: 'Smithing Trim',
    },
    smithing_transform: {
        id: 'app.lab.recipe-generator.type.smithing-transform',
        defaultMessage: 'Smithing Transform',
    },
})

export const categoryMessages = defineMessages({
    food: { id: 'app.lab.recipe-generator.category.food', defaultMessage: 'Food' },
    blocks: { id: 'app.lab.recipe-generator.category.blocks', defaultMessage: 'Blocks' },
    misc: { id: 'app.lab.recipe-generator.category.misc', defaultMessage: 'Misc' },
    equipment: { id: 'app.lab.recipe-generator.category.equipment', defaultMessage: 'Equipment' },
    building: { id: 'app.lab.recipe-generator.category.building', defaultMessage: 'Building' },
    redstone: { id: 'app.lab.recipe-generator.category.redstone', defaultMessage: 'Redstone' },
})
