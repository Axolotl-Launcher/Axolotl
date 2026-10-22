export type JavaVersionId =
    | '1.12'
    | '1.13'
    | '1.14'
    | '1.15'
    | '1.16'
    | '1.17'
    | '1.18'
    | '1.19'
    | '1.20'
    | '1.21'
    | '1.21.2'
    | '1.21.4'
    | '1.21.5'
    | '1.21.6'
    | '1.21.7'
    | '1.21.9'
    | '1.21.11'
    | '26.1'
    | '26.2'
    | '26.3'

export type RecipeType =
    | 'brewing'
    | 'crafting'
    | 'smelting'
    | 'blasting'
    | 'smoking'
    | 'campfire_cooking'
    | 'stonecutter'
    | 'smithing'
    | 'smithing_trim'
    | 'smithing_transform'

export type PackFormatVersion = number | [number, number]

export type RecipeSlot =
    | 'crafting.1'
    | 'crafting.2'
    | 'crafting.3'
    | 'crafting.4'
    | 'crafting.5'
    | 'crafting.6'
    | 'crafting.7'
    | 'crafting.8'
    | 'crafting.9'
    | 'crafting.result'
    | 'cooking.ingredient'
    | 'cooking.result'
    | 'stonecutter.ingredient'
    | 'stonecutter.result'
    | 'smithing.template'
    | 'smithing.base'
    | 'smithing.addition'
    | 'smithing.result'
    | 'brewing.input'
    | 'brewing.reagent'
    | 'brewing.result'

export type SlotValue =
    | { kind: 'item'; id: string; count?: number }
    | { kind: 'custom_item'; uid: string; count?: number }
    | { kind: 'vanilla_tag'; id: string }
    | { kind: 'custom_tag'; uid: string }

export type CustomItem = {
    uid: string
    id: string
    name: string
    texture: string
    createdAt: string
}

export type TagValue = {
    type: 'item' | 'tag'
    id: string
}

export type CustomTag = {
    uid: string
    id: string
    values: TagValue[]
}

export type BrewingState = {
    inputPotion: string
    reagentPotion: string
    outputPotion: string
    inputPredicate: string
    reagentPredicate: string
    outputComponents: string
}

export type RecipeState = {
    id: string
    recipeType: RecipeType
    group: string
    category: string
    showNotification: boolean
    nameMode: 'auto' | 'manual'
    name: string
    slots: Partial<Record<RecipeSlot, SlotValue>>
    crafting: {
        shapeless: boolean
        keepWhitespace: boolean
        twoByTwo: boolean
    }
    brewing?: BrewingState
    cooking: {
        time: number | null
        experience: number
    }
    smithing: {
        trimPattern: string
    }
}

export type RecipeSlotContext = {
    itemsById: Record<string, { id: string; name: string; texture: string | null }>
    customItemsByUid: Record<string, CustomItem>
    customTagsByUid: Record<string, CustomTag>
    vanillaTags: Record<string, string[]>
}

export type RecipeGeneratorStore = {
    version: 2
    selectedVersion: JavaVersionId
    recipes: RecipeState[]
    selectedRecipeId: string
    customItems: CustomItem[]
    customTags: CustomTag[]
}
