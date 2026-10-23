<script setup lang="ts">
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import PencilIcon from '@modrinth/assets/icons/pencil.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import {
    defineMessages,
    type MessageDescriptor,
    useVIntl,
} from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import {
    computed,
    inject,
    onBeforeUnmount,
    reactive,
    ref,
    shallowRef,
    useTemplateRef,
    watch,
} from 'vue'

import { createRequestGeneration } from '../../lib/request-generation.ts'
import NewModal from '../../overlays/PreviewModal.vue'
import potionItems from './recipe-generator/assets/vanilla/potion-items.json'
import BrewingRecipeOptions from './recipe-generator/components/BrewingRecipeOptions.vue'
import InstanceExportModal from './recipe-generator/components/InstanceExportModal.vue'
import ItemPalette, { type PaletteEntry } from './recipe-generator/components/ItemPalette.vue'
import RecipeGeneratorCopyrightModal from './recipe-generator/components/RecipeGeneratorCopyrightModal.vue'
import RecipeItemIcon from './recipe-generator/components/RecipeItemIcon.vue'
import RecipeSlotGrid from './recipe-generator/components/RecipeSlotGrid.vue'
import TagPalette from './recipe-generator/components/TagPalette.vue'
import { drawCountOnCanvas } from './recipe-generator/count-display.ts'
import {
    createDatapackDescription,
    createDatapackFileName,
    createDatapackFiles,
    type DatapackRecipe,
    type PackFile,
    saveDatapackAs,
    saveJsonFile,
} from './recipe-generator/datapack.ts'
import { getSlotDisplay, type SlotDisplay } from './recipe-generator/display.ts'
import { parseIdentifier } from './recipe-generator/identifier.ts'
import { useDatapackExport } from './recipe-generator/instance-export.ts'
import { resolveRecipeItemName } from './recipe-generator/item-names.ts'
import { categoryMessages, recipeTypeMessages } from './recipe-generator/messages.ts'
import { getCurrentRecipeName } from './recipe-generator/naming.ts'
import {
    createDefaultRecipeState,
    loadRecipeProject,
    RECIPE_PROJECT_STORAGE_KEY,
    switchRecipeProjectVersion,
} from './recipe-generator/project.ts'
import { CRAFTING_GRID_SLOTS, generateJavaRecipe } from './recipe-generator/recipe-engine.ts'
import {
    getRecipeLayout,
    RECIPE_IMAGE_HEIGHT,
    RECIPE_IMAGE_WIDTH,
    type RecipeLayoutSlotBox,
} from './recipe-generator/recipe-layouts.ts'
import {
    buildSlotContext,
    type LoadedVersionResources,
    loadVersionResources,
    TEXTURE_ATLAS,
} from './recipe-generator/resources.ts'
import type {
    CustomItem,
    CustomTag,
    JavaVersionId,
    RecipeSlot,
    RecipeSlotContext,
    RecipeState,
    RecipeType,
    SlotValue,
} from './recipe-generator/types.ts'
import { useResultCountWheel } from './recipe-generator/useResultCountWheel.ts'
import { type RecipeIssueCode, validateRecipe } from './recipe-generator/validation.ts'
import {
    DEFAULT_COOKING_TIME,
    defaultCookingTime,
    getRecipeCategoryOptions,
    getSupportedRecipeTypes,
    JAVA_VERSIONS,
    supportsCustomTags,
    supportsRecipeCategory,
    supportsShowNotification,
    supportsSmithingTrimPattern,
    supportsVanillaTagList,
} from './recipe-generator/versions.ts'

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage, locale } = useVIntl()
const { exportDatapackToWorld } = useDatapackExport()

const store = reactive(loadRecipeProject())
const replayOnboarding = inject<(mode: 'brewing') => Promise<void>>('replayOnboarding')
let projectSaveTimer: ReturnType<typeof setTimeout> | undefined
watch(
    store,
    () => {
        if (projectSaveTimer) clearTimeout(projectSaveTimer)
        projectSaveTimer = setTimeout(() => {
            try {
                localStorage.setItem(RECIPE_PROJECT_STORAGE_KEY, JSON.stringify(store))
            } catch (error) {
                handleError(error)
            }
        }, 350)
    },
    { deep: true },
)

const resources = shallowRef<LoadedVersionResources | null>(null)
const loadingResources = ref(false)
const resourceError = ref('')
const resourceRequests = createRequestGeneration()
const rightTab = ref<'items' | 'tags'>('items')
const pendingDatapack = ref<{ files: PackFile[]; fileName: string } | null>(null)
const customItemDraft = reactive({ uid: '', id: '', name: '', texture: '' })
const customItemModal = useTemplateRef<InstanceType<typeof NewModal>>('customItemModal')
const copyrightModal =
    useTemplateRef<InstanceType<typeof RecipeGeneratorCopyrightModal>>('copyrightModal')
const instanceExportModal =
    useTemplateRef<InstanceType<typeof InstanceExportModal>>('instanceExportModal')

const messages = defineMessages({
    issueInvalidCount: {
        id: 'app.lab.recipe-generator.issue.invalid-result-count',
        defaultMessage: 'Output count must be an integer from 1 to 99.',
    },
    brewingCount: { id: 'app.lab.recipe-generator.brewing.count', defaultMessage: 'Output count' },
    issueInvalidBrewingJson: {
        id: 'app.lab.recipe-generator.issue.invalid-brewing-json',
        defaultMessage: 'Brewing predicates and components must contain valid JSON objects.',
    },
    title: { id: 'app.lab.recipe-generator.title', defaultMessage: 'Recipe generator' },
    version: { id: 'app.lab.recipe-generator.version', defaultMessage: 'Version' },
    versionLabel: {
        id: 'app.lab.recipe-generator.version-label',
        defaultMessage: 'Java {version}',
    },
    copyright: {
        id: 'app.lab.recipe-generator.copyright.open',
        defaultMessage: 'Copyright and attribution',
    },
    loadingResources: {
        id: 'app.lab.recipe-generator.loading-resources',
        defaultMessage: 'Loading version data',
    },
    resourceError: {
        id: 'app.lab.recipe-generator.resource-error',
        defaultMessage: 'Unable to load version data.',
    },
    recipesTitle: {
        id: 'app.lab.recipe-generator.recipes.title',
        defaultMessage: 'Recipes',
    },
    recipeName: {
        id: 'app.lab.recipe-generator.recipes.name',
        defaultMessage: 'Name: {name}',
    },
    newRecipe: { id: 'app.lab.recipe-generator.recipes.new', defaultMessage: 'New recipe' },
    cloneRecipe: { id: 'app.lab.recipe-generator.recipes.clone', defaultMessage: 'Clone recipe' },
    deleteRecipe: {
        id: 'app.lab.recipe-generator.recipes.delete',
        defaultMessage: 'Delete recipe',
    },
    exportDatapack: {
        id: 'app.lab.recipe-generator.recipes.export-datapack',
        defaultMessage: 'Export datapack',
    },
    exportDatapackDone: {
        id: 'app.lab.recipe-generator.recipes.export-datapack-done',
        defaultMessage: 'Datapack exported',
    },
    exportDatapackInvalid: {
        id: 'app.lab.recipe-generator.recipes.export-datapack-invalid',
        defaultMessage: 'Fix recipe errors before exporting.',
    },
    recipeType: { id: 'app.lab.recipe-generator.recipe-type', defaultMessage: 'Recipe type' },
    optionsTitle: {
        id: 'app.lab.recipe-generator.options.title',
        defaultMessage: 'Options',
    },
    previewTitle: { id: 'app.lab.recipe-generator.preview.title', defaultMessage: 'Preview' },
    outputTitle: { id: 'app.lab.recipe-generator.output.title', defaultMessage: 'JSON output' },
    saveJson: {
        id: 'app.lab.recipe-generator.output.save-json',
        defaultMessage: 'Save JSON',
    },
    jsonSaved: {
        id: 'app.lab.recipe-generator.output.saved',
        defaultMessage: 'JSON saved',
    },
    copyPreviewImage: {
        id: 'app.lab.recipe-generator.preview.copy-image',
        defaultMessage: 'Copy image',
    },
    previewImageCopied: {
        id: 'app.lab.recipe-generator.preview.image-copied',
        defaultMessage: 'Preview image copied',
    },
    datapackSaved: {
        id: 'app.lab.recipe-generator.save-as-done',
        defaultMessage: 'Datapack saved',
    },
    shapeless: {
        id: 'app.lab.recipe-generator.options.shapeless',
        defaultMessage: 'Shapeless crafting',
    },
    twoByTwo: {
        id: 'app.lab.recipe-generator.options.two-by-two',
        defaultMessage: '2x2 grid',
    },
    keepWhitespace: {
        id: 'app.lab.recipe-generator.options.keep-whitespace',
        defaultMessage: 'Keep exact slot positions',
    },
    experience: { id: 'app.lab.recipe-generator.options.experience', defaultMessage: 'Experience' },
    cookingTime: {
        id: 'app.lab.recipe-generator.options.cooking-time',
        defaultMessage: 'Cooking time',
    },
    defaultTime: {
        id: 'app.lab.recipe-generator.options.default-time',
        defaultMessage: 'Default time',
    },
    group: { id: 'app.lab.recipe-generator.options.group', defaultMessage: 'Group' },
    category: { id: 'app.lab.recipe-generator.options.category', defaultMessage: 'Category' },
    categoryNone: {
        id: 'app.lab.recipe-generator.options.category-none',
        defaultMessage: 'None',
    },
    showNotification: {
        id: 'app.lab.recipe-generator.options.show-notification',
        defaultMessage: 'Show recipe notification',
    },
    fileName: { id: 'app.lab.recipe-generator.options.file-name', defaultMessage: 'File name' },
    nameModeAuto: {
        id: 'app.lab.recipe-generator.options.name-mode-auto',
        defaultMessage: 'Automatic',
    },
    nameModeManual: {
        id: 'app.lab.recipe-generator.options.name-mode-manual',
        defaultMessage: 'Manual',
    },
    manualNamePlaceholder: {
        id: 'app.lab.recipe-generator.options.manual-name-placeholder',
        defaultMessage: 'recipe_name',
    },
    autoName: {
        id: 'app.lab.recipe-generator.options.auto-name',
        defaultMessage: 'Auto name: {name}',
    },
    clearSlots: { id: 'app.lab.recipe-generator.slots.clear', defaultMessage: 'Clear slots' },
    itemsTab: { id: 'app.lab.recipe-generator.panel.items', defaultMessage: 'Items' },
    tagsTab: { id: 'app.lab.recipe-generator.panel.tags', defaultMessage: 'Tags' },
    customItemsTitle: {
        id: 'app.lab.recipe-generator.items.custom-title',
        defaultMessage: 'Custom items',
    },
    addCustomItem: {
        id: 'app.lab.recipe-generator.items.add-custom',
        defaultMessage: 'Add custom item',
    },
    editCustomItem: {
        id: 'app.lab.recipe-generator.items.edit-custom',
        defaultMessage: 'Edit custom item',
    },
    customItemId: {
        id: 'app.lab.recipe-generator.items.custom-id',
        defaultMessage: 'Item ID',
    },
    customItemIdPlaceholder: {
        id: 'app.lab.recipe-generator.items.custom-id-placeholder',
        defaultMessage: 'minecraft:item_id',
    },
    customItemName: {
        id: 'app.lab.recipe-generator.items.custom-name',
        defaultMessage: 'Display name',
    },
    customItemTexture: {
        id: 'app.lab.recipe-generator.items.custom-texture',
        defaultMessage: 'Texture URL',
    },
    customTexturePlaceholder: {
        id: 'app.lab.recipe-generator.items.custom-texture-placeholder',
        defaultMessage: 'https://example.com/texture.png',
    },
    save: { id: 'app.lab.recipe-generator.save', defaultMessage: 'Save' },
    cancel: { id: 'app.lab.recipe-generator.cancel', defaultMessage: 'Cancel' },
    invalidCustomItem: {
        id: 'app.lab.recipe-generator.items.invalid-custom',
        defaultMessage: 'Enter an item ID.',
    },
    customItemSaved: {
        id: 'app.lab.recipe-generator.items.custom-saved',
        defaultMessage: 'Custom item saved',
    },
    customItemDeleted: {
        id: 'app.lab.recipe-generator.items.custom-deleted',
        defaultMessage: 'Custom item deleted',
    },
    deleteCustomItem: {
        id: 'app.lab.recipe-generator.items.delete-custom',
        defaultMessage: 'Delete custom item',
    },
    editCustomItemAction: {
        id: 'app.lab.recipe-generator.items.edit-custom-action',
        defaultMessage: 'Edit custom item',
    },
    gridFull: {
        id: 'app.lab.recipe-generator.slots.grid-full',
        defaultMessage: 'The grid is full. Clear a slot first.',
    },
    recipeErrors: {
        id: 'app.lab.recipe-generator.validation.summary',
        defaultMessage: '{count} issue',
    },
    recipeErrorsPlural: {
        id: 'app.lab.recipe-generator.validation.summary-plural',
        defaultMessage: '{count} issues',
    },
    issueUnsupportedType: {
        id: 'app.lab.recipe-generator.validation.unsupported-type',
        defaultMessage: 'This recipe type is not available in the selected version.',
    },
    issueMissingIngredient: {
        id: 'app.lab.recipe-generator.validation.missing-ingredient',
        defaultMessage: 'Add an ingredient.',
    },
    issueMissingResult: {
        id: 'app.lab.recipe-generator.validation.missing-result',
        defaultMessage: 'Add a result item.',
    },
    issueMissingTemplate: {
        id: 'app.lab.recipe-generator.validation.missing-template',
        defaultMessage: 'Add a template item.',
    },
    issueMissingBase: {
        id: 'app.lab.recipe-generator.validation.missing-base',
        defaultMessage: 'Add a base item.',
    },
    issueMissingAddition: {
        id: 'app.lab.recipe-generator.validation.missing-addition',
        defaultMessage: 'Add an addition item.',
    },
    issueMissingTrimPattern: {
        id: 'app.lab.recipe-generator.validation.missing-trim-pattern',
        defaultMessage: 'Add a trim pattern.',
    },
    issueTagInResult: {
        id: 'app.lab.recipe-generator.validation.tag-in-result',
        defaultMessage: 'Result slots cannot contain tags.',
    },
    issueMissingCustomItem: {
        id: 'app.lab.recipe-generator.validation.missing-custom-item',
        defaultMessage: 'The recipe references a deleted custom item.',
    },
    issueMissingCustomTag: {
        id: 'app.lab.recipe-generator.validation.missing-custom-tag',
        defaultMessage: 'The recipe references a deleted custom tag.',
    },
    issueInvalidIdentifier: {
        id: 'app.lab.recipe-generator.validation.invalid-identifier',
        defaultMessage: 'An item identifier is invalid.',
    },
    issueTagsNotSupported: {
        id: 'app.lab.recipe-generator.validation.tags-not-supported',
        defaultMessage: 'Item tags are not supported in this version.',
    },
    trimPatternLabel: {
        id: 'app.lab.recipe-generator.options.trim-pattern',
        defaultMessage: 'Trim pattern',
    },
    jsonPlaceholder: {
        id: 'app.lab.recipe-generator.output.placeholder',
        defaultMessage: 'Recipe JSON appears here',
    },
})

const issueMessages: Record<RecipeIssueCode, MessageDescriptor> = {
    'unsupported-type': messages.issueUnsupportedType,
    'missing-ingredient': messages.issueMissingIngredient,
    'missing-result': messages.issueMissingResult,
    'missing-template': messages.issueMissingTemplate,
    'missing-base': messages.issueMissingBase,
    'missing-addition': messages.issueMissingAddition,
    'missing-trim-pattern': messages.issueMissingTrimPattern,
    'tag-in-result': messages.issueTagInResult,
    'missing-custom-item': messages.issueMissingCustomItem,
    'missing-custom-tag': messages.issueMissingCustomTag,
    'invalid-identifier': messages.issueInvalidIdentifier,
    'tags-not-supported': messages.issueTagsNotSupported,
    'invalid-brewing-json': messages.issueInvalidBrewingJson,
    'invalid-result-count': messages.issueInvalidCount,
}

const currentRecipe = computed(
    () => store.recipes.find((recipe) => recipe.id === store.selectedRecipeId) ?? store.recipes[0],
)
const slotContext = computed<RecipeSlotContext | null>(() => {
    if (!resources.value) return null
    const context = buildSlotContext(store.customItems, store.customTags, resources.value)
    const itemsById: RecipeSlotContext['itemsById'] = {}
    for (const item of Object.values(context.itemsById)) {
        itemsById[item.id] = {
            ...item,
            name: resolveRecipeItemName(item.id, locale.value, item.name),
        }
    }
    return { ...context, itemsById }
})
const generatedJson = computed(() => {
    if (!currentRecipe.value || !slotContext.value) return null
    try {
        return generateJavaRecipe(currentRecipe.value, store.selectedVersion, slotContext.value)
    } catch {
        return null
    }
})
const jsonText = computed(() =>
    generatedJson.value ? JSON.stringify(generatedJson.value, null, 2) : '',
)
const naming = computed(() =>
    currentRecipe.value && slotContext.value
        ? getCurrentRecipeName(currentRecipe.value, store.recipes, slotContext.value)
        : null,
)
const issues = computed(() =>
    currentRecipe.value && slotContext.value
        ? validateRecipe(currentRecipe.value, store.selectedVersion, slotContext.value)
        : [],
)
const availableTypes = computed(() => getSupportedRecipeTypes(store.selectedVersion))
const categoryOptions = computed(
    () => getRecipeCategoryOptions(currentRecipe.value?.recipeType ?? 'crafting') ?? [],
)
const showCategory = computed(() =>
    currentRecipe.value
        ? supportsRecipeCategory(store.selectedVersion, currentRecipe.value.recipeType)
        : false,
)
const showNotificationOption = computed(() => {
    const recipe = currentRecipe.value
    return recipe
        ? supportsShowNotification(
              store.selectedVersion,
              recipe.recipeType,
              recipe.crafting.shapeless,
          )
        : false
})
const showTrimPattern = computed(
    () =>
        currentRecipe.value?.recipeType === 'smithing_trim' &&
        supportsSmithingTrimPattern(store.selectedVersion),
)
const showVanillaTags = computed(() => supportsVanillaTagList(store.selectedVersion))
const showCustomTags = computed(() => supportsCustomTags(store.selectedVersion))
const paletteEntries = computed<PaletteEntry[]>(() => {
    if (!resources.value) return []
    const entries: PaletteEntry[] = resources.value.items.map((item) => {
        const name = resolveRecipeItemName(item.id, locale.value, item.name)
        return {
            key: `item:${item.id}`,
            name,
            id: item.id,
            display: { label: name, texture: item.texture, isTag: false },
            value: { kind: 'item', id: item.id },
        }
    })
    for (const item of store.customItems) {
        entries.push({
            key: `custom:${item.uid}`,
            name: item.name,
            id: item.id,
            display: { label: item.name, texture: item.texture || null, isTag: false },
            value: { kind: 'custom_item', uid: item.uid },
        })
    }
    return entries
})
const autoPlaceSlots = computed<RecipeSlot[]>(() => {
    switch (currentRecipe.value?.recipeType) {
        case 'crafting': {
            const recipe = currentRecipe.value
            if (recipe?.crafting.twoByTwo) {
                return CRAFTING_GRID_SLOTS.filter((slot) => !TWO_BY_TWO_DISABLED_SLOTS.has(slot))
            }
            return [...CRAFTING_GRID_SLOTS]
        }
        case 'smelting':
        case 'blasting':
        case 'smoking':
        case 'campfire_cooking':
            return ['cooking.ingredient']
        case 'brewing':
            return ['brewing.input', 'brewing.reagent']
        case 'stonecutter':
            return ['stonecutter.ingredient']
        case 'smithing':
        case 'smithing_trim':
        case 'smithing_transform':
            return ['smithing.template', 'smithing.base', 'smithing.addition']
        default:
            return []
    }
})

const RECIPE_SLOT_MIME_TYPE = 'application/x-axolotl-recipe-slot'
const TWO_BY_TWO_DISABLED_SLOTS = new Set<RecipeSlot>([
    'crafting.3',
    'crafting.6',
    'crafting.7',
    'crafting.8',
    'crafting.9',
])
const PREVIEW_ICON_SCALE = 0.9
const TRIM_PATTERNS = [
    'coast',
    'sentry',
    'dune',
    'wild',
    'ward',
    'eye',
    'vex',
    'tide',
    'snout',
    'rib',
    'spire',
    'wayfinder',
    'shaper',
    'silence',
    'raiser',
    'host',
    'flow',
    'bolt',
] as const

type LayoutSlotRender = {
    layoutSlot: RecipeSlot
    targetSlot: RecipeSlot
    box: RecipeLayoutSlotBox
    disabled: boolean
}

type TrimPatternOption = {
    pattern: string
    id: string
    display: SlotDisplay
}

const recipeLayout = computed(() =>
    currentRecipe.value ? getRecipeLayout(currentRecipe.value.recipeType) : null,
)
const layoutStageRef = useTemplateRef<HTMLDivElement>('layoutStage')
const layoutStageWidth = ref(0)
let layoutStageObserver: ResizeObserver | null = null
const recipeEditorRef = useTemplateRef<HTMLElement>('recipeEditor')
const recipePaletteMaxHeight = ref('')
let recipeEditorObserver: ResizeObserver | null = null

const layoutSlots = computed<LayoutSlotRender[]>(() => {
    const layout = recipeLayout.value
    if (!layout) return []
    const twoByTwoCrafting =
        currentRecipe.value?.recipeType === 'crafting' && currentRecipe.value.crafting.twoByTwo
    return (Object.entries(layout.slots) as [RecipeSlot, RecipeLayoutSlotBox][]).map(
        ([layoutSlot, box]) => ({
            layoutSlot,
            targetSlot: layoutTargetSlot(layoutSlot),
            box,
            disabled: twoByTwoCrafting && TWO_BY_TWO_DISABLED_SLOTS.has(layoutSlot),
        }),
    )
})

const barrierDisplay = computed<SlotDisplay | null>(() =>
    slotContext.value
        ? getSlotDisplay({ kind: 'item', id: 'minecraft:barrier' }, slotContext.value)
        : null,
)

const resultCountSlot = computed<RecipeSlot | null>(() => {
    switch (currentRecipe.value?.recipeType) {
        case 'crafting':
            return 'crafting.result'
        case 'brewing':
            return 'brewing.result'
        case 'stonecutter':
            return 'stonecutter.result'
        default:
            return null
    }
})
const { hint: resultWheelHintRef, onWheel: onResultSlotWheel } = useResultCountWheel({
    getSlot: () => resultCountSlot.value,
    getMaximum: () => (currentRecipe.value?.recipeType === 'brewing' ? 99 : 64),
    getValue: () => {
        const slot = resultCountSlot.value
        return slot ? currentRecipe.value?.slots[slot] : undefined
    },
    getCount: () => {
        const slot = resultCountSlot.value
        const value = slot ? currentRecipe.value?.slots[slot] : undefined
        return value && (value.kind === 'item' || value.kind === 'custom_item') && value.count
            ? value.count
            : 1
    },
    setCount: (count) => {
        const slot = resultCountSlot.value
        if (slot) setSlotCount(slot, count)
    },
})

const trimPatternOptions = computed<TrimPatternOption[]>(() => {
    const ctx = slotContext.value
    if (!ctx) return []
    return TRIM_PATTERNS.map((pattern) => {
        const id = `minecraft:${pattern}_armor_trim_smithing_template`
        const item = ctx.itemsById[id]
        const label = item ? resolveRecipeItemName(item.id, locale.value, item.name) : pattern
        return {
            pattern,
            id,
            display: item
                ? { label, texture: item.texture, isTag: false }
                : { label, texture: null, isTag: false },
        }
    })
})

watch(
    layoutStageRef,
    (element) => {
        layoutStageObserver?.disconnect()
        layoutStageObserver = null
        layoutStageWidth.value = 0
        if (!element || typeof ResizeObserver === 'undefined') return
        layoutStageObserver = new ResizeObserver(([entry]) => {
            layoutStageWidth.value = entry.contentRect.width
        })
        layoutStageObserver.observe(element)
        layoutStageWidth.value = element.getBoundingClientRect().width
    },
    { flush: 'post' },
)

watch(
    recipeEditorRef,
    (element) => {
        recipeEditorObserver?.disconnect()
        recipeEditorObserver = null
        recipePaletteMaxHeight.value = ''
        if (!element) return
        const updatePaletteMaxHeight = () => {
            const height = element.getBoundingClientRect().height
            recipePaletteMaxHeight.value = height ? `${height}px` : ''
        }
        updatePaletteMaxHeight()
        if (typeof ResizeObserver === 'undefined') return
        recipeEditorObserver = new ResizeObserver(updatePaletteMaxHeight)
        recipeEditorObserver.observe(element)
    },
    { flush: 'post' },
)

onBeforeUnmount(() => {
    layoutStageObserver?.disconnect()
    recipeEditorObserver?.disconnect()
    if (projectSaveTimer) clearTimeout(projectSaveTimer)
    try {
        localStorage.setItem(RECIPE_PROJECT_STORAGE_KEY, JSON.stringify(store))
    } catch (error) {
        handleError(error)
    }
})

watch(
    () => store.selectedVersion,
    (version, previous) => {
        if (previous === version) return
        if (previous) switchRecipeProjectVersion(store, version, previous)
        loadResources(version)
    },
    { immediate: true },
)

watch(
    [() => currentRecipe.value?.id, () => currentRecipe.value?.crafting.twoByTwo],
    () => {
        const recipe = currentRecipe.value
        if (!recipe || recipe.recipeType !== 'crafting' || !recipe.crafting.twoByTwo) return
        for (const slot of TWO_BY_TWO_DISABLED_SLOTS) {
            Reflect.deleteProperty(recipe.slots, slot)
        }
    },
    { immediate: true },
)

async function loadResources(version: JavaVersionId) {
    const request = resourceRequests.begin(version)
    loadingResources.value = true
    resourceError.value = ''
    try {
        const result = await loadVersionResources(version)
        if (
            !resourceRequests.isCurrent(request, store.selectedVersion) ||
            version !== store.selectedVersion
        )
            return
        resources.value = result
    } catch (error) {
        if (
            !resourceRequests.isCurrent(request, store.selectedVersion) ||
            version !== store.selectedVersion
        )
            return
        resources.value = null
        resourceError.value = error instanceof Error ? error.message : String(error)
    } finally {
        if (resourceRequests.isCurrent(request, store.selectedVersion))
            loadingResources.value = false
    }
}

function versionLabel(version: JavaVersionId) {
    return formatMessage(messages.versionLabel, { version })
}

function recipeTypeLabel(type: RecipeType) {
    return formatMessage(recipeTypeMessages[type])
}

function categoryLabel(category: string) {
    return category
        ? formatMessage(categoryMessages[category as keyof typeof categoryMessages])
        : formatMessage(messages.categoryNone)
}

function nameModeLabel(mode: 'auto' | 'manual') {
    return mode === 'auto'
        ? formatMessage(messages.nameModeAuto)
        : formatMessage(messages.nameModeManual)
}

function setRecipeType(type: RecipeType) {
    const recipe = currentRecipe.value
    if (!recipe || recipe.recipeType === type) return
    recipe.recipeType = type
    recipe.slots = {}
    recipe.category = ''
}

function setTwoByTwo(value: boolean) {
    const recipe = currentRecipe.value
    if (!recipe || recipe.recipeType !== 'crafting') return
    recipe.crafting.twoByTwo = value
    if (value) {
        for (const slot of TWO_BY_TWO_DISABLED_SLOTS) {
            Reflect.deleteProperty(recipe.slots, slot)
        }
    }
}

function newRecipe() {
    const type = availableTypes.value[0] ?? 'crafting'
    const recipe = createDefaultRecipeState(type)
    store.recipes.push(recipe)
    store.selectedRecipeId = recipe.id
}

function cloneRecipe(recipeId: string) {
    const recipe = store.recipes.find((recipe) => recipe.id === recipeId)
    if (!recipe) return
    const copy: RecipeState = JSON.parse(JSON.stringify(recipe))
    copy.id = crypto.randomUUID()
    copy.nameMode = 'auto'
    copy.name = ''
    store.recipes.push(copy)
    store.selectedRecipeId = copy.id
}

function deleteRecipe(recipeId: string) {
    const index = store.recipes.findIndex((recipe) => recipe.id === recipeId)
    if (index < 0) return
    if (store.recipes.length <= 1) {
        const recipe = createDefaultRecipeState(availableTypes.value[0] ?? 'crafting')
        store.recipes = [recipe]
        store.selectedRecipeId = recipe.id
        return
    }
    store.recipes.splice(index, 1)
    if (store.selectedRecipeId === recipeId) {
        store.selectedRecipeId = store.recipes[Math.max(0, index - 1)]?.id ?? store.recipes[0].id
    }
}

function setSlot(slot: RecipeSlot, value: SlotValue | undefined) {
    const recipe = currentRecipe.value
    if (!recipe) return
    if (
        recipe.recipeType === 'crafting' &&
        recipe.crafting.twoByTwo &&
        TWO_BY_TWO_DISABLED_SLOTS.has(slot)
    ) {
        return
    }
    if (value) recipe.slots[slot] = { ...value }
    else Reflect.deleteProperty(recipe.slots, slot)
}

function layoutTargetSlot(layoutSlot: RecipeSlot): RecipeSlot {
    return currentRecipe.value?.recipeType === 'smithing_trim' && layoutSlot === 'smithing.result'
        ? 'smithing.base'
        : layoutSlot
}

function slotBoxStyle(box: RecipeLayoutSlotBox) {
    return {
        left: `${(box.x1 / RECIPE_IMAGE_WIDTH) * 100}%`,
        top: `${(box.y1 / RECIPE_IMAGE_HEIGHT) * 100}%`,
        width: `${((box.x2 - box.x1) / RECIPE_IMAGE_WIDTH) * 100}%`,
        height: `${((box.y2 - box.y1) / RECIPE_IMAGE_HEIGHT) * 100}%`,
    }
}

function slotIconSize(box: RecipeLayoutSlotBox) {
    if (!layoutStageWidth.value) return 32
    const backgroundSize = Math.min(box.x2 - box.x1, box.y2 - box.y1)
    return Math.max(1, Math.round((backgroundSize / RECIPE_IMAGE_WIDTH) * layoutStageWidth.value))
}

function scaledSlotIconSize(box: RecipeLayoutSlotBox) {
    return Math.max(1, Math.round(slotIconSize(box) * PREVIEW_ICON_SCALE))
}

function slotDisplayFor(slot: RecipeSlot): SlotDisplay | null {
    const recipe = currentRecipe.value
    const ctx = slotContext.value
    const display = recipe && ctx ? getSlotDisplay(recipe.slots[slot], ctx) : null
    const value = recipe?.slots[slot]
    const potion =
        slot === 'brewing.input'
            ? recipe?.brewing?.inputPotion
            : slot === 'brewing.reagent'
              ? recipe?.brewing?.reagentPotion
              : slot === 'brewing.result'
                ? recipe?.brewing?.outputPotion
                : undefined
    const variant =
        value?.kind === 'item' && potion
            ? potionItems.find((item) => item.id === value.id && item.potion === potion)
            : undefined
    return display && variant ? { ...display, texture: variant.texture } : display
}

function isSlotValue(value: unknown): value is SlotValue {
    if (typeof value !== 'object' || value === null) return false
    const candidate = value as { kind?: unknown; id?: unknown; uid?: unknown }
    switch (candidate.kind) {
        case 'item':
        case 'vanilla_tag':
            return typeof candidate.id === 'string'
        case 'custom_item':
        case 'custom_tag':
            return typeof candidate.uid === 'string'
        default:
            return false
    }
}

function parseSlotValue(raw: string): SlotValue | null {
    if (!raw) return null
    try {
        const parsed: unknown = JSON.parse(raw)
        return isSlotValue(parsed) ? parsed : null
    } catch {
        return null
    }
}

function hasRecipePayload(event: DragEvent) {
    const types = event.dataTransfer?.types
    if (!types) return false
    const typeList = Array.from(types)
    return (
        !typeList.includes('Files') &&
        (typeList.includes(RECIPE_SLOT_MIME_TYPE) || typeList.includes('text/plain'))
    )
}

function onHotspotClick(slot: RecipeSlot) {
    const recipe = currentRecipe.value
    if (!recipe || !recipe.slots[slot]) return
    setSlot(slot, undefined)
}

function onSlotDropEvent(event: Event, slot: RecipeSlot) {
    const detail = (event as CustomEvent<{ value?: unknown }>).detail
    if (!detail || !isSlotValue(detail.value)) return
    setSlot(slot, detail.value)
}

function onHotspotDragOver(event: DragEvent) {
    const dataTransfer = event.dataTransfer
    if (!dataTransfer) return
    if (!hasRecipePayload(event)) {
        dataTransfer.dropEffect = 'none'
        return
    }
    event.preventDefault()
    dataTransfer.dropEffect = 'copy'
}

function onHotspotDrop(event: DragEvent, slot: RecipeSlot) {
    const dataTransfer = event.dataTransfer
    if (!dataTransfer || !hasRecipePayload(event)) return
    const raw = dataTransfer.getData(RECIPE_SLOT_MIME_TYPE) || dataTransfer.getData('text/plain')
    const value = parseSlotValue(raw)
    if (!value) return
    event.preventDefault()
    setSlot(slot, value)
}

function selectTrimPattern(id: string) {
    const recipe = currentRecipe.value
    if (recipe) recipe.smithing.trimPattern = id
}

function setSlotCount(slot: RecipeSlot, count: number) {
    const value = currentRecipe.value?.slots[slot]
    if (value && (value.kind === 'item' || value.kind === 'custom_item')) {
        value.count = count
    }
}

function resultWheelHint(slot: RecipeSlot) {
    return slot === resultCountSlot.value ? resultWheelHintRef.value : undefined
}

function clearSlots() {
    const recipe = currentRecipe.value
    if (recipe) recipe.slots = {}
}

function placeFromPalette(value: SlotValue) {
    const recipe = currentRecipe.value
    if (!recipe) return
    const firstEmpty = autoPlaceSlots.value.find((slot) => !recipe.slots[slot])
    if (firstEmpty) {
        recipe.slots[firstEmpty] = { ...value }
    } else {
        addNotification({ type: 'info', title: formatMessage(messages.gridFull) })
    }
}

const cookingTimeIsDefault = computed(() => {
    const recipe = currentRecipe.value
    return !recipe || recipe.cooking.time === null
})

function toggleDefaultTime(value: boolean) {
    const recipe = currentRecipe.value
    if (!recipe) return
    if (value) recipe.cooking.time = null
    else if (recipe.recipeType in DEFAULT_COOKING_TIME) {
        recipe.cooking.time = defaultCookingTime(
            recipe.recipeType as keyof typeof DEFAULT_COOKING_TIME,
            store.selectedVersion,
        )
    }
}

function openCustomItemModal(item?: CustomItem) {
    customItemDraft.uid = item?.uid ?? ''
    customItemDraft.id = item?.id ?? ''
    customItemDraft.name = item?.name ?? ''
    customItemDraft.texture = item?.texture ?? ''
    customItemModal.value?.show()
}

function saveCustomItem() {
    const id = customItemDraft.id.trim()
    if (!id) {
        addNotification({ type: 'error', title: formatMessage(messages.invalidCustomItem) })
        return
    }
    if (customItemDraft.uid) {
        const index = store.customItems.findIndex((item) => item.uid === customItemDraft.uid)
        if (index >= 0) {
            store.customItems[index] = {
                ...store.customItems[index],
                id,
                name: customItemDraft.name.trim() || id,
                texture: customItemDraft.texture.trim(),
            }
        }
    } else {
        store.customItems.push({
            uid: crypto.randomUUID(),
            id,
            name: customItemDraft.name.trim() || id,
            texture: customItemDraft.texture.trim(),
            createdAt: new Date().toISOString(),
        })
    }
    customItemModal.value?.hide()
    addNotification({ type: 'success', title: formatMessage(messages.customItemSaved) })
}

function deleteCustomItem(uid: string) {
    store.customItems = store.customItems.filter((item) => item.uid !== uid)
    removeCustomReferences(uid)
    addNotification({ type: 'success', title: formatMessage(messages.customItemDeleted) })
}

function addCustomTag(tag: CustomTag) {
    store.customTags.push(tag)
}

function updateCustomTag(tag: CustomTag) {
    const index = store.customTags.findIndex((entry) => entry.uid === tag.uid)
    if (index >= 0) store.customTags[index] = tag
}

function deleteCustomTag(uid: string) {
    store.customTags = store.customTags.filter((tag) => tag.uid !== uid)
    removeCustomReferences(uid)
}

function removeCustomReferences(uid: string) {
    for (const recipe of store.recipes) {
        for (const [slot, value] of Object.entries(recipe.slots) as [RecipeSlot, SlotValue][]) {
            if (
                (value.kind === 'custom_item' || value.kind === 'custom_tag') &&
                value.uid === uid
            ) {
                Reflect.deleteProperty(recipe.slots, slot)
            }
        }
    }
}

async function saveCurrentJson() {
    if (!generatedJson.value || !naming.value) return
    try {
        const path = await saveJsonFile(generatedJson.value, `${naming.value.resolvedName}.json`)
        if (path) addNotification({ type: 'success', title: formatMessage(messages.jsonSaved) })
    } catch (error) {
        handleError(error)
    }
}

const previewImageCache = new Map<string, Promise<HTMLImageElement>>()
const PREVIEW_EXPORT_SCALE = 4

function loadPreviewImage(url: string): Promise<HTMLImageElement> {
    const cached = previewImageCache.get(url)
    if (cached) return cached
    const promise = new Promise<HTMLImageElement>((resolve, reject) => {
        const image = new Image()
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error(`Unable to load preview image: ${url}`))
        image.src = url
    })
    previewImageCache.set(url, promise)
    return promise
}

async function drawPreviewIcon(
    context: CanvasRenderingContext2D,
    atlasImage: HTMLImageElement,
    customImages: Map<string, HTMLImageElement>,
    display: SlotDisplay,
    box: RecipeLayoutSlotBox,
    scale: number,
): Promise<{ x: number; y: number; size: number } | null> {
    const size = Math.round(Math.min(box.x2 - box.x1, box.y2 - box.y1) * scale)
    const x = Math.round(((box.x1 + box.x2) / 2) * scale - size / 2)
    const y = Math.round(((box.y1 + box.y2) / 2) * scale - size / 2)
    const iconCanvas = document.createElement('canvas')
    iconCanvas.width = size
    iconCanvas.height = size
    const iconContext = iconCanvas.getContext('2d')
    if (!iconContext) return null
    iconContext.imageSmoothingEnabled = false
    const region = display.texture ? TEXTURE_ATLAS.layout[display.texture] : undefined
    if (region) {
        const [ux, uy, uw, uh] = region
        iconContext.drawImage(atlasImage, ux, uy, uw, uh, 0, 0, size, size)
    } else if (display.texture) {
        try {
            let customImage = customImages.get(display.texture)
            if (!customImage) {
                customImage = await loadPreviewImage(display.texture)
                customImages.set(display.texture, customImage)
            }
            iconContext.drawImage(customImage, 0, 0, size, size)
        } catch {
            // Unreachable custom textures are skipped.
            return null
        }
    } else {
        return null
    }
    context.drawImage(iconCanvas, x, y)
    return { x, y, size }
}

async function createLayoutPreviewPngBlob(): Promise<Blob> {
    const layout = recipeLayout.value
    if (!layout) throw new Error('No recipe layout available')
    const canvas = document.createElement('canvas')
    canvas.width = RECIPE_IMAGE_WIDTH * PREVIEW_EXPORT_SCALE
    canvas.height = RECIPE_IMAGE_HEIGHT * PREVIEW_EXPORT_SCALE
    const context = canvas.getContext('2d')
    if (!context) throw new Error('Unable to create a canvas context')
    context.imageSmoothingEnabled = false

    const background = await loadPreviewImage(layout.image)
    context.drawImage(
        background,
        0,
        0,
        RECIPE_IMAGE_WIDTH * PREVIEW_EXPORT_SCALE,
        RECIPE_IMAGE_HEIGHT * PREVIEW_EXPORT_SCALE,
    )

    const atlasImage = await loadPreviewImage(TEXTURE_ATLAS.url)
    const customImages = new Map<string, HTMLImageElement>()

    for (const entry of layoutSlots.value) {
        const { box, targetSlot } = entry
        if (entry.disabled) {
            context.fillStyle = 'rgba(0, 0, 0, 0.58)'
            context.fillRect(
                box.x1 * PREVIEW_EXPORT_SCALE,
                box.y1 * PREVIEW_EXPORT_SCALE,
                (box.x2 - box.x1) * PREVIEW_EXPORT_SCALE,
                (box.y2 - box.y1) * PREVIEW_EXPORT_SCALE,
            )
            const barrier = barrierDisplay.value
            if (barrier?.texture) {
                await drawPreviewIcon(
                    context,
                    atlasImage,
                    customImages,
                    barrier,
                    box,
                    PREVIEW_EXPORT_SCALE,
                )
            }
            continue
        }
        const display = slotDisplayFor(targetSlot)
        const drawnIcon = display?.texture
            ? await drawPreviewIcon(
                  context,
                  atlasImage,
                  customImages,
                  display,
                  box,
                  PREVIEW_EXPORT_SCALE,
              )
            : null
        if (display?.count && display.count > 1 && drawnIcon) {
            drawCountOnCanvas(context, display.count, drawnIcon.size, drawnIcon.x, drawnIcon.y)
        }
    }

    return await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob((result) => {
            if (result) resolve(result)
            else reject(new Error('Unable to encode preview PNG'))
        }, 'image/png')
    })
}

async function copyPreviewImage() {
    if (!currentRecipe.value || !slotContext.value) return
    try {
        const blob = await createLayoutPreviewPngBlob()
        await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])
        addNotification({ type: 'success', title: formatMessage(messages.previewImageCopied) })
    } catch (error) {
        handleError(error)
    }
}

async function saveAs() {
    if (!slotContext.value) return
    try {
        const files = buildDatapackFiles()
        if (!files) return
        const path = await saveDatapackAs(
            files,
            pendingDatapack.value?.fileName ?? createDatapackFileName(store.selectedVersion),
        )
        if (path) addNotification({ type: 'success', title: formatMessage(messages.datapackSaved) })
    } catch (error) {
        handleError(error)
    }
}

async function exportDatapack() {
    if (!buildDatapackFiles()) return
    instanceExportModal.value?.show()
}

function buildDatapackFiles(): PackFile[] | null {
    if (!slotContext.value) return null
    const invalidCount = store.recipes.filter(
        (recipe) => validateRecipe(recipe, store.selectedVersion, slotContext.value!).length > 0,
    ).length
    if (invalidCount > 0) {
        addNotification({ type: 'error', title: formatMessage(messages.exportDatapackInvalid) })
        return null
    }
    const namingMap = new Map(
        store.recipes.map((recipe) => [
            recipe.id,
            getCurrentRecipeName(recipe, store.recipes, slotContext.value!).resolvedName,
        ]),
    )
    const recipes: DatapackRecipe[] = store.recipes.map((recipe) => ({
        name: namingMap.get(recipe.id) ?? 'recipe',
        json: generateJavaRecipe(recipe, store.selectedVersion, slotContext.value!),
    }))
    const tags = store.customTags.map((tag) => {
        const ref = parseIdentifier(tag.id)
        return {
            namespace: ref.namespace,
            id: ref.id,
            values: tag.values.map((value) => (value.type === 'tag' ? `#${value.id}` : value.id)),
        }
    })
    try {
        const productNames = store.recipes.map((recipe) => sidebarTitle(recipe))
        const description = createDatapackDescription(productNames)
        const files = createDatapackFiles(store.selectedVersion, recipes, tags, description)
        const fileName = createDatapackFileName(store.selectedVersion)
        pendingDatapack.value = { files, fileName }
        return files
    } catch (error) {
        handleError(error)
        return null
    }
}

async function installDatapackToWorld(target: { instanceId: string; worldPath: string }) {
    const pending = pendingDatapack.value
    if (!pending) return
    await exportDatapackToWorld(
        target.instanceId,
        target.worldPath,
        pending.files,
        pending.fileName,
    )
    addNotification({ type: 'success', title: formatMessage(messages.exportDatapackDone) })
}

function sidebarTitle(recipe: RecipeState): string {
    const resolved = slotContext.value
        ? getCurrentRecipeName(recipe, store.recipes, slotContext.value).sidebarTitle
        : ''
    return resolved || formatMessage(recipeTypeMessages[recipe.recipeType])
}

function issueLabel(code: RecipeIssueCode) {
    return formatMessage(issueMessages[code])
}

function slotEditorSlots(type: RecipeType): RecipeSlot[] {
    switch (type) {
        case 'brewing':
            return ['brewing.input', 'brewing.reagent', 'brewing.result']
        case 'crafting':
            return [...CRAFTING_GRID_SLOTS, 'crafting.result']
        case 'smelting':
        case 'blasting':
        case 'smoking':
        case 'campfire_cooking':
            return ['cooking.ingredient', 'cooking.result']
        case 'stonecutter':
            return ['stonecutter.ingredient', 'stonecutter.result']
        case 'smithing':
        case 'smithing_transform':
            return ['smithing.template', 'smithing.base', 'smithing.addition', 'smithing.result']
        case 'smithing_trim':
            return ['smithing.template', 'smithing.base', 'smithing.addition']
    }
}
</script>

<template>
    <main class="recipe-generator-page" data-onboarding-id="recipe-generator-workspace">
        <header class="recipe-generator-header">
            <h1 class="recipe-generator-title">{{ formatMessage(messages.title) }}</h1>
            <div class="recipe-header-actions">
                <div class="recipe-version-control">
                    <span class="recipe-field-label">{{ formatMessage(messages.version) }}</span>
                    <Combobox
                        v-model="store.selectedVersion"
                        :options="
                            JAVA_VERSIONS.map((version) => ({
                                value: version.id,
                                label: versionLabel(version.id),
                            }))
                        "
                        :max-height="405"
                        class="recipe-version-dropdown"
                    />
                </div>
                <Button type="outlined" size="2xs" @click="copyrightModal?.show()"
                    >{{ formatMessage(messages.copyright) }}
                </Button>
            </div>
        </header>

        <div v-if="loadingResources" class="recipe-resource-state">
            <span class="recipe-resource-spinner" aria-hidden="true"></span>
            {{ formatMessage(messages.loadingResources) }}
        </div>
        <div v-else-if="resourceError" class="recipe-resource-state recipe-resource-error text-red">
            {{ formatMessage(messages.resourceError) }}
            <span class="font-mono text-xs">{{ resourceError }}</span>
        </div>

        <div v-else-if="resources && slotContext" class="recipe-workbench">
            <aside class="lab-panel recipe-sidebar">
                <div class="recipe-sidebar-heading">
                    <h2>{{ formatMessage(messages.recipesTitle) }}</h2>
                    <Button type="colored" color="brand" size="2xs" @click="newRecipe"
                        ><PlusIcon />{{ formatMessage(messages.newRecipe) }}</Button
                    >
                </div>
                <div class="recipe-sidebar-list">
                    <div
                        v-for="recipe in store.recipes"
                        :key="recipe.id"
                        class="recipe-sidebar-row focus-within:border-brand"
                        :class="{ active: recipe.id === store.selectedRecipeId }"
                    >
                        <button
                            type="button"
                            class="recipe-sidebar-select"
                            @click="store.selectedRecipeId = recipe.id"
                        >
                            <strong>{{ sidebarTitle(recipe) }}</strong>
                            <small>{{
                                formatMessage(messages.recipeName, {
                                    name: getCurrentRecipeName(recipe, store.recipes, slotContext)
                                        .resolvedName,
                                })
                            }}</small>
                        </button>
                        <span class="recipe-row-actions">
                            <button
                                type="button"
                                :title="formatMessage(messages.cloneRecipe)"
                                :aria-label="formatMessage(messages.cloneRecipe)"
                                @click.stop="cloneRecipe(recipe.id)"
                            >
                                <PlusIcon />
                            </button>
                            <button
                                type="button"
                                :title="formatMessage(messages.deleteRecipe)"
                                :aria-label="formatMessage(messages.deleteRecipe)"
                                @click.stop="deleteRecipe(recipe.id)"
                            >
                                <TrashIcon />
                            </button>
                        </span>
                    </div>
                </div>
                <div class="recipe-sidebar-footer">
                    <Button
                        type="colored"
                        color="brand"
                        class="recipe-export-datapack w-full"
                        @click="exportDatapack"
                        ><DownloadIcon />{{ formatMessage(messages.exportDatapack) }}
                    </Button>
                </div>
            </aside>

            <section ref="recipeEditor" class="lab-panel recipe-editor overflow-visible">
                <div class="lab-panel-section recipe-options-section p-4">
                    <div class="recipe-section-heading">
                        <h2>{{ formatMessage(messages.optionsTitle) }}</h2>
                    </div>
                    <div class="recipe-option-grid">
                        <div class="recipe-field">
                            <span class="recipe-field-label">{{
                                formatMessage(messages.recipeType)
                            }}</span>
                            <Combobox
                                :model-value="currentRecipe?.recipeType"
                                :options="
                                    availableTypes.map((value) => ({
                                        value,
                                        label: recipeTypeLabel(value),
                                    }))
                                "
                                width="full"
                                @update:model-value="setRecipeType(String($event) as RecipeType)"
                            />
                        </div>
                        <div class="recipe-field">
                            <span class="recipe-field-label">{{
                                formatMessage(messages.group)
                            }}</span>
                            <StyledInput
                                v-model="currentRecipe.group"
                                :placeholder="formatMessage(messages.group)"
                                size="small"
                            />
                        </div>
                        <div v-if="showCategory" class="recipe-field">
                            <span class="recipe-field-label">{{
                                formatMessage(messages.category)
                            }}</span>
                            <Combobox
                                v-model="currentRecipe.category"
                                :options="
                                    categoryOptions.map((value) => ({
                                        value,
                                        label: categoryLabel(value),
                                    }))
                                "
                                width="full"
                            />
                        </div>
                        <div class="recipe-field">
                            <span class="recipe-field-label">{{
                                formatMessage(messages.fileName)
                            }}</span>
                            <Combobox
                                v-model="currentRecipe.nameMode"
                                :options="
                                    ['auto', 'manual'].map((value) => ({
                                        value,
                                        label: nameModeLabel(value),
                                    }))
                                "
                                width="full"
                            />
                        </div>
                        <div v-if="currentRecipe.nameMode === 'manual'" class="recipe-field">
                            <span class="recipe-field-label">{{
                                formatMessage(messages.fileName)
                            }}</span>
                            <StyledInput
                                v-model="currentRecipe.name"
                                :placeholder="formatMessage(messages.manualNamePlaceholder)"
                                size="small"
                            />
                        </div>
                    </div>

                    <div v-if="currentRecipe?.recipeType === 'crafting'" class="recipe-option-grid">
                        <Checkbox
                            :model-value="currentRecipe.crafting.shapeless"
                            :label="formatMessage(messages.shapeless)"
                            @update:model-value="currentRecipe.crafting.shapeless = Boolean($event)"
                        />
                        <Checkbox
                            :model-value="currentRecipe.crafting.twoByTwo"
                            :label="formatMessage(messages.twoByTwo)"
                            @update:model-value="setTwoByTwo(Boolean($event))"
                        />
                        <Checkbox
                            :model-value="currentRecipe.crafting.keepWhitespace"
                            :disabled="currentRecipe.crafting.shapeless"
                            :label="formatMessage(messages.keepWhitespace)"
                            @update:model-value="
                                currentRecipe.crafting.keepWhitespace = Boolean($event)
                            "
                        />
                    </div>

                    <div
                        v-if="
                            currentRecipe?.recipeType === 'smelting' ||
                            currentRecipe?.recipeType === 'blasting' ||
                            currentRecipe?.recipeType === 'smoking' ||
                            currentRecipe?.recipeType === 'campfire_cooking'
                        "
                        class="recipe-option-grid"
                    >
                        <div class="recipe-field">
                            <span class="recipe-field-label">{{
                                formatMessage(messages.experience)
                            }}</span>
                            <StyledInput
                                :model-value="String(currentRecipe.cooking.experience)"
                                type="number"
                                :min="0"
                                :step="0.05"
                                size="small"
                                @update:model-value="
                                    currentRecipe.cooking.experience = Math.max(
                                        0,
                                        Number($event) || 0,
                                    )
                                "
                            />
                        </div>
                        <div class="recipe-field">
                            <span class="recipe-field-label">{{
                                formatMessage(messages.cookingTime)
                            }}</span>
                            <StyledInput
                                :model-value="
                                    currentRecipe.cooking.time === null
                                        ? String(
                                              defaultCookingTime(
                                                  currentRecipe.recipeType as keyof typeof DEFAULT_COOKING_TIME,
                                                  store.selectedVersion,
                                              ),
                                          )
                                        : String(currentRecipe.cooking.time)
                                "
                                :disabled="cookingTimeIsDefault"
                                type="number"
                                :min="1"
                                size="small"
                                @update:model-value="
                                    currentRecipe.cooking.time = Math.max(
                                        1,
                                        Math.round(Number($event) || 1),
                                    )
                                "
                            />
                        </div>
                        <Checkbox
                            :model-value="cookingTimeIsDefault"
                            :label="formatMessage(messages.defaultTime)"
                            @update:model-value="toggleDefaultTime(Boolean($event))"
                        />
                    </div>

                    <label
                        v-if="currentRecipe.recipeType === 'brewing'"
                        class="flex items-center justify-between gap-3 text-sm"
                        ><span>{{ formatMessage(messages.brewingCount) }}</span
                        ><StyledInput
                            :model-value="
                                String(
                                    currentRecipe.slots['brewing.result'] &&
                                        'count' in currentRecipe.slots['brewing.result']!
                                        ? (currentRecipe.slots['brewing.result']!.count ?? 1)
                                        : 1,
                                )
                            "
                            type="number"
                            :min="1"
                            :max="99"
                            :disabled="!currentRecipe.slots['brewing.result']"
                            @update:model-value="
                                setSlotCount(
                                    'brewing.result',
                                    Math.max(1, Math.min(99, Math.floor(Number($event) || 1))),
                                )
                            "
                    /></label>
                    <BrewingRecipeOptions
                        v-if="currentRecipe.recipeType === 'brewing' && currentRecipe.brewing"
                        v-model="currentRecipe.brewing"
                        @guide="replayOnboarding?.('brewing')"
                    />
                    <label v-if="showNotificationOption" class="recipe-toggle-row">
                        <span>{{ formatMessage(messages.showNotification) }}</span>
                        <Toggle v-model="currentRecipe.showNotification" small />
                    </label>
                    <span v-if="naming" class="recipe-auto-name">
                        {{ formatMessage(messages.autoName, { name: naming.resolvedName }) }}
                    </span>
                </div>

                <div class="lab-panel-section p-4">
                    <div class="recipe-section-heading">
                        <h2>{{ formatMessage(messages.previewTitle) }}</h2>
                        <div class="recipe-preview-actions">
                            <Button
                                type="quiet"
                                size="2xs"
                                class="recipe-clear-slots"
                                @click="clearSlots"
                                >{{ formatMessage(messages.clearSlots) }}
                            </Button>
                            <Button
                                type="colored"
                                color="brand"
                                size="2xs"
                                class="recipe-copy-preview"
                                :disabled="!currentRecipe || !slotContext"
                                @click="copyPreviewImage"
                                ><ClipboardCopyIcon />{{ formatMessage(messages.copyPreviewImage) }}
                            </Button>
                        </div>
                    </div>

                    <div
                        v-if="recipeLayout"
                        ref="layoutStage"
                        class="recipe-layout-stage"
                        :style="{ backgroundImage: `url('${recipeLayout.image}')` }"
                    >
                        <template v-for="entry in layoutSlots" :key="entry.layoutSlot">
                            <div
                                v-if="entry.disabled"
                                class="recipe-layout-barrier"
                                :style="slotBoxStyle(entry.box)"
                                aria-hidden="true"
                            >
                                <span v-if="barrierDisplay" class="recipe-layout-barrier-icon flex">
                                    <RecipeItemIcon
                                        :display="barrierDisplay"
                                        :atlas="TEXTURE_ATLAS"
                                        :size="slotIconSize(entry.box) + 2"
                                        :show-count="false"
                                    />
                                </span>
                            </div>
                            <div
                                v-else
                                v-tooltip="resultWheelHint(entry.targetSlot)"
                                class="recipe-layout-hotspot"
                                :data-recipe-slot="entry.targetSlot"
                                :style="slotBoxStyle(entry.box)"
                                @click="onHotspotClick(entry.targetSlot)"
                                @axolotl-recipe-slot-drop="
                                    onSlotDropEvent($event, entry.targetSlot)
                                "
                                @dragover="onHotspotDragOver"
                                @drop="onHotspotDrop($event, entry.targetSlot)"
                                @wheel="onResultSlotWheel(entry.targetSlot, $event)"
                            >
                                <span
                                    v-if="slotDisplayFor(entry.targetSlot)"
                                    class="recipe-layout-icon"
                                >
                                    <RecipeItemIcon
                                        :display="slotDisplayFor(entry.targetSlot)"
                                        :atlas="TEXTURE_ATLAS"
                                        :size="scaledSlotIconSize(entry.box) + 2"
                                    />
                                </span>
                            </div>
                        </template>
                    </div>
                    <RecipeSlotGrid
                        v-if="!recipeLayout && currentRecipe.recipeType === 'crafting'"
                        :slots="[...CRAFTING_GRID_SLOTS, 'crafting.result']"
                        :values="currentRecipe.slots"
                        :ctx="slotContext"
                        :atlas="TEXTURE_ATLAS"
                        variant="crafting"
                        :two-by-two="currentRecipe.crafting.twoByTwo"
                        @update-slot="setSlot"
                        @update-count="setSlotCount"
                    />
                    <RecipeSlotGrid
                        v-else-if="!recipeLayout"
                        :slots="slotEditorSlots(currentRecipe.recipeType)"
                        :values="currentRecipe.slots"
                        :ctx="slotContext"
                        :atlas="TEXTURE_ATLAS"
                        variant="row"
                        @update-slot="setSlot"
                        @update-count="setSlotCount"
                    />

                    <div v-if="showTrimPattern" class="recipe-trim-pattern-selector">
                        <span class="recipe-field-label">{{
                            formatMessage(messages.trimPatternLabel)
                        }}</span>
                        <div class="recipe-trim-pattern-grid">
                            <button
                                v-for="option in trimPatternOptions"
                                :key="option.pattern"
                                type="button"
                                class="recipe-trim-pattern-option"
                                :class="{
                                    active: currentRecipe.smithing.trimPattern === option.id,
                                }"
                                :title="option.display.label"
                                :aria-label="option.display.label"
                                @click="selectTrimPattern(option.id)"
                            >
                                <RecipeItemIcon
                                    :display="option.display"
                                    :atlas="TEXTURE_ATLAS"
                                    :size="30"
                                    :show-count="false"
                                />
                                <span>{{ option.display.label }}</span>
                            </button>
                        </div>
                    </div>

                    <div v-if="issues.length" class="recipe-issues">
                        <strong>
                            {{
                                formatMessage(
                                    issues.length === 1
                                        ? messages.recipeErrors
                                        : messages.recipeErrorsPlural,
                                    { count: issues.length },
                                )
                            }}
                        </strong>
                        <ul>
                            <li v-for="(issue, index) in issues" :key="index">
                                {{ issueLabel(issue.code) }}
                            </li>
                        </ul>
                    </div>
                </div>

                <div class="lab-panel-section p-4">
                    <div class="recipe-section-heading">
                        <h2>{{ formatMessage(messages.outputTitle) }}</h2>
                        <div class="recipe-output-actions">
                            <Button
                                type="colored"
                                color="brand"
                                size="2xs"
                                :disabled="!jsonText"
                                @click="saveCurrentJson"
                                ><SaveIcon />{{ formatMessage(messages.saveJson) }}
                            </Button>
                        </div>
                    </div>
                    <textarea
                        readonly
                        :value="jsonText"
                        :aria-label="formatMessage(messages.outputTitle)"
                        class="recipe-json-output"
                        :placeholder="formatMessage(messages.jsonPlaceholder)"
                    ></textarea>
                </div>
            </section>

            <aside
                class="lab-panel recipe-palette"
                :style="
                    recipePaletteMaxHeight
                        ? { '--recipe-palette-max-height': recipePaletteMaxHeight }
                        : undefined
                "
            >
                <div class="recipe-palette-heading">
                    <div class="recipe-tag-tabs" role="tablist">
                        <button
                            type="button"
                            class="recipe-tag-tab"
                            role="tab"
                            :aria-selected="rightTab === 'items'"
                            :class="{ active: rightTab === 'items' }"
                            @click="rightTab = 'items'"
                        >
                            {{ formatMessage(messages.itemsTab) }}
                        </button>
                        <button
                            type="button"
                            class="recipe-tag-tab"
                            role="tab"
                            :aria-selected="rightTab === 'tags'"
                            :class="{ active: rightTab === 'tags' }"
                            @click="rightTab = 'tags'"
                        >
                            {{ formatMessage(messages.tagsTab) }}
                        </button>
                    </div>
                    <Button
                        v-if="rightTab === 'items'"
                        type="outlined"
                        size="2xs"
                        @click="openCustomItemModal()"
                        ><PlusIcon />{{ formatMessage(messages.addCustomItem) }}
                    </Button>
                </div>

                <ItemPalette
                    v-if="rightTab === 'items'"
                    :entries="paletteEntries"
                    :atlas="TEXTURE_ATLAS"
                    :loading="loadingResources"
                    @pick="placeFromPalette"
                />
                <TagPalette
                    v-else
                    :vanilla-tags="showVanillaTags ? resources.vanillaTags : {}"
                    :custom-tags="showCustomTags ? store.customTags : []"
                    :ctx="slotContext"
                    :atlas="TEXTURE_ATLAS"
                    @pick="placeFromPalette"
                    @add-custom-tag="addCustomTag"
                    @update-custom-tag="updateCustomTag"
                    @delete-custom-tag="deleteCustomTag"
                />
                <div
                    v-if="rightTab === 'items' && store.customItems.length"
                    class="recipe-custom-item-list"
                >
                    <h3>{{ formatMessage(messages.customItemsTitle) }}</h3>
                    <div
                        v-for="item in store.customItems"
                        :key="item.uid"
                        class="recipe-custom-item-row"
                    >
                        <RecipeItemIcon
                            :display="{
                                label: item.name,
                                texture: item.texture || null,
                                isTag: false,
                            }"
                            :atlas="TEXTURE_ATLAS"
                            :size="24"
                            :show-count="false"
                        />
                        <span class="min-w-0 flex-1 truncate text-xs">{{ item.name }}</span>
                        <button
                            type="button"
                            class="recipe-delete-custom"
                            :title="formatMessage(messages.editCustomItemAction)"
                            :aria-label="formatMessage(messages.editCustomItemAction)"
                            @click="openCustomItemModal(item)"
                        >
                            <PencilIcon />
                        </button>
                        <button
                            type="button"
                            class="recipe-delete-custom"
                            :title="formatMessage(messages.deleteCustomItem)"
                            :aria-label="formatMessage(messages.deleteCustomItem)"
                            @click="deleteCustomItem(item.uid)"
                        >
                            <TrashIcon />
                        </button>
                    </div>
                </div>
            </aside>
        </div>

        <NewModal
            ref="customItemModal"
            :header="
                formatMessage(
                    customItemDraft.uid ? messages.editCustomItem : messages.addCustomItem,
                )
            "
        >
            <div class="custom-item-form">
                <label class="recipe-field">
                    <span>{{ formatMessage(messages.customItemId) }}</span>
                    <StyledInput
                        v-model="customItemDraft.id"
                        :placeholder="formatMessage(messages.customItemIdPlaceholder)"
                    />
                </label>
                <label class="recipe-field">
                    <span>{{ formatMessage(messages.customItemName) }}</span>
                    <StyledInput v-model="customItemDraft.name" />
                </label>
                <label class="recipe-field">
                    <span>{{ formatMessage(messages.customItemTexture) }}</span>
                    <StyledInput
                        v-model="customItemDraft.texture"
                        :placeholder="formatMessage(messages.customTexturePlaceholder)"
                    />
                </label>
                <div class="flex justify-end gap-2">
                    <Button type="outlined" size="2xs" @click="customItemModal?.hide()">{{
                        formatMessage(messages.cancel)
                    }}</Button>
                    <Button type="colored" color="brand" size="2xs" @click="saveCustomItem">{{
                        formatMessage(messages.save)
                    }}</Button>
                </div>
            </div>
        </NewModal>
        <RecipeGeneratorCopyrightModal ref="copyrightModal" />
        <InstanceExportModal
            ref="instanceExportModal"
            :on-install="installDatapackToWorld"
            @save-as="saveAs"
        />
    </main>
</template>

<style scoped>
.recipe-generator-page {
    --recipe-title-size: 1.5rem;
    --recipe-panel-title-size: 1rem;
    --recipe-label-size: 0.7rem;
    --recipe-body-size: 0.875rem;
    --recipe-mono-size: 0.75rem;
    --color-surface-1: var(--surface-1);
    --color-surface-2: var(--surface-2);
    --color-surface-3: var(--surface-3);
    --color-surface-4: var(--surface-4);
    --color-surface-5: var(--surface-5);
    display: flex;
    width: 100%;
    max-width: 90rem;
    min-height: 0;
    flex-direction: column;
    gap: 1.25rem;
    margin-inline: auto;
    padding: 1.5rem;
    container-type: inline-size;
    container-name: recipe-generator-page;
}

:global(.recipe-slot-drag-ghost) {
    --color-surface-2: var(--surface-2);
}

.recipe-generator-header {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
}

.recipe-generator-title {
    margin: 0;
    min-width: 0;
    color: var(--color-text-primary);
    font-size: var(--recipe-title-size);
    font-weight: 700;
    line-height: 1.25;
}

.recipe-header-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.5rem;
}

.recipe-version-control {
    position: relative;
    z-index: 0;
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 0.35rem;
    color: var(--color-text-tertiary);
    font-size: var(--recipe-label-size);
    font-weight: 700;
}

.recipe-version-control:focus-within {
    z-index: 2;
}

.recipe-version-control :deep([data-combobox]) {
    width: min(20rem, 384px);
}

.recipe-resource-state {
    display: flex;
    min-height: 14rem;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 0.65rem;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-lg);
    background: var(--color-surface-2);
    color: var(--color-text-tertiary);
    font-size: var(--recipe-body-size);
    text-align: center;
}

.recipe-resource-spinner {
    width: 1.5rem;
    height: 1.5rem;
    border: 2px solid var(--color-surface-5);
    border-top-color: var(--color-brand);
    border-radius: 50%;
    animation: recipe-spin 0.8s linear infinite;
}

@keyframes recipe-spin {
    to {
        transform: rotate(360deg);
    }
}

.recipe-workbench {
    display: grid;
    grid-template-columns: minmax(14rem, 0.55fr) minmax(26rem, 1.45fr) minmax(18rem, 0.9fr);
    align-items: start;
    gap: 1.25rem;
}

.lab-panel {
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-lg);
    background: var(--surface-2);
    box-shadow: var(--shadow-card);
}

.lab-panel-section + .lab-panel-section {
    border-top: 1px solid var(--color-surface-5);
}

.recipe-sidebar {
    display: flex;
    min-width: 0;
    flex-direction: column;
}

.recipe-sidebar-heading {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    border-bottom: 1px solid var(--color-surface-5);
    padding: 0.85rem 1rem;
}

.recipe-sidebar-heading h2,
.recipe-section-heading h2 {
    margin: 0;
    color: var(--color-text-primary);
    font-size: var(--recipe-panel-title-size);
    font-weight: 700;
}

.recipe-sidebar-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.75rem;
}

.recipe-sidebar-row {
    display: flex;
    min-width: 0;
    align-items: stretch;
    gap: 0.25rem;
    overflow: hidden;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-4);
    transition:
        background-color 0.15s ease,
        border-color 0.15s ease;
}

.recipe-sidebar-row.active {
    border-color: var(--color-brand);
    background: var(--color-brand-highlight);
}

.recipe-sidebar-select {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.15rem;
    border: 0;
    background: transparent;
    padding: 0.5rem 0.55rem;
    color: var(--color-text-primary);
    cursor: pointer;
    text-align: left;
}

.recipe-sidebar-select:focus-visible {
    outline: none;
}

.recipe-sidebar-select strong {
    overflow: hidden;
    font-size: 0.8rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.recipe-sidebar-select small {
    overflow: hidden;
    color: var(--color-text-tertiary);
    font-family: monospace;
    font-size: 0.7rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.recipe-row-actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 0.15rem;
    padding: 0.3rem 0.35rem;
}

.recipe-row-actions button,
.recipe-delete-custom {
    display: inline-flex;
    width: 1.75rem;
    height: 1.75rem;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    padding: 0;
    color: var(--color-text-tertiary);
    cursor: pointer;
}

.recipe-row-actions button:hover,
.recipe-delete-custom:hover {
    background: var(--color-red-highlight);
    color: var(--color-red);
}

.recipe-row-actions svg,
.recipe-delete-custom svg {
    width: 0.9rem;
    height: 0.9rem;
}

.recipe-sidebar-footer {
    border-top: 1px solid var(--color-surface-5);
    padding: 0.85rem 1rem;
}

.recipe-editor :deep(.recipe-crafting-editor .recipe-result-column) {
    position: relative;
    padding-left: 2rem;
}

.recipe-editor :deep(.recipe-crafting-editor .recipe-result-column)::before {
    content: '→';
    position: absolute;
    left: 0.5rem;
    top: 50%;
    transform: translateY(-50%);
    color: var(--color-text-tertiary);
    font-size: 1.1rem;
    line-height: 1;
    pointer-events: none;
}

.recipe-options-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.recipe-section-heading {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
}

.recipe-preview-actions {
    display: flex;
    min-width: 0;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 0.35rem;
}

.recipe-option-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 10.5rem), 1fr));
    align-items: end;
    gap: 0.75rem 1rem;
}

.recipe-field {
    position: relative;
    z-index: 0;
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 0.3rem;
    color: var(--color-text-tertiary);
}

.recipe-field-label {
    font-size: var(--recipe-label-size);
    font-weight: 700;
    line-height: 1.2;
}

.recipe-field:focus-within {
    z-index: 2;
}

.recipe-field :deep([data-combobox]) {
    width: 100%;
}

.recipe-toggle-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--color-text-primary);
    font-size: var(--recipe-body-size);
}

.recipe-layout-stage {
    position: relative;
    width: 100%;
    margin-top: 0.75rem;
    aspect-ratio: 696 / 292;
    background-repeat: no-repeat;
    background-position: center;
    background-size: 100% 100%;
}

.recipe-layout-hotspot {
    position: absolute;
    margin: 0;
    border: 0;
    background: transparent;
    padding: 0;
    cursor: pointer;
}

.recipe-layout-barrier {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: rgb(0 0 0 / 0.58);
    pointer-events: none;
}

.recipe-layout-icon {
    position: absolute;
    left: 50%;
    top: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transform: translate(-50%, -50%);
    pointer-events: none;
}

.recipe-trim-pattern-selector {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    margin-top: 0.85rem;
}

.recipe-trim-pattern-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 4.5rem), 1fr));
    gap: 0.4rem;
}

.recipe-trim-pattern-option {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 0.2rem;
    overflow: hidden;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-4);
    padding: 0.3rem 0.2rem;
    color: var(--color-text-tertiary);
    cursor: pointer;
    transition:
        background-color 0.15s ease,
        border-color 0.15s ease;
}

.recipe-trim-pattern-option:hover,
.recipe-trim-pattern-option:focus-visible {
    border-color: var(--color-brand);
    background: var(--color-surface-3);
    outline: none;
}

.recipe-trim-pattern-option.active {
    border-color: var(--color-brand);
    background: var(--color-brand-highlight);
    color: var(--color-text-primary);
}

.recipe-trim-pattern-option > span {
    max-width: 100%;
    overflow: hidden;
    font-size: 0.58rem;
    line-height: 1.15;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.recipe-auto-name {
    align-self: flex-start;
    color: var(--color-text-tertiary);
    font-family: monospace;
    font-size: var(--recipe-mono-size);
}

.recipe-issues {
    margin-top: 0.85rem;
    border: 1px solid var(--color-red);
    border-radius: var(--radius-md);
    background: var(--color-red-highlight);
    padding: 0.6rem 0.75rem;
    color: var(--color-red);
    font-size: 0.8rem;
}

.recipe-issues ul {
    margin: 0.35rem 0 0;
    padding-left: 1.1rem;
}

.recipe-output-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.5rem;
}

.recipe-json-output {
    width: 100%;
    min-height: 12rem;
    margin-top: 0.75rem;
    resize: vertical;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-1);
    padding: 0.7rem;
    color: var(--color-text-primary);
    font-family: monospace;
    font-size: var(--recipe-mono-size);
    line-height: 1.5;
    outline: none;
}

.recipe-palette {
    display: flex;
    min-width: 0;
    min-height: 0;
    height: auto;
    max-height: var(--recipe-palette-max-height, 61rem);
    flex-direction: column;
    container-type: inline-size;
    container-name: recipe-palette;
}

.recipe-palette-heading {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    border-bottom: 1px solid var(--color-surface-5);
    padding: 0.75rem;
}

:deep(.recipe-palette-grid) {
    grid-template-columns: repeat(4, minmax(0, 1fr));
    grid-auto-rows: 4.25rem;
    gap: 0.5rem;
    align-content: stretch;
}

:deep(.recipe-palette-item) {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.2rem;
    border: 0 !important;
    box-shadow: var(--shadow-card);
    padding: 0.2rem 0.1rem;
}

:deep(.recipe-palette-name) {
    width: 100%;
    min-width: 0;
    color: var(--color-text-primary);
    font-size: 0.7rem;
    line-height: 1.1;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.recipe-tag-tabs {
    display: flex;
    min-width: 0;
    gap: 0.25rem;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-3);
    padding: 0.2rem;
}

.recipe-tag-tab {
    flex: 1;
    border: 0;
    border-radius: calc(var(--radius-sm) - 1px);
    background: transparent;
    padding: 0.4rem 0.55rem;
    color: var(--color-text-tertiary);
    cursor: pointer;
    font-size: 0.75rem;
    font-weight: 700;
    white-space: nowrap;
}

.recipe-tag-tab.active {
    background: var(--color-brand);
    color: var(--color-accent-contrast);
}

.recipe-custom-item-list {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    border-top: 1px solid var(--color-surface-5);
    padding: 0.75rem;
}

.recipe-custom-item-list h3 {
    margin: 0;
    color: var(--color-text-tertiary);
    font-size: var(--recipe-label-size);
    text-transform: uppercase;
}

.recipe-custom-item-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface-4);
    padding: 0.3rem 0.4rem;
}

.custom-item-form {
    display: flex;
    width: min(26rem, calc(1280px - 3rem));
    flex-direction: column;
    gap: 0.75rem;
}

@media (max-width: 68rem) {
    .recipe-workbench {
        grid-template-columns: minmax(0, 1fr);
    }

    .recipe-sidebar {
        position: static;
        height: auto;
        min-height: 0;
        max-height: none;
    }

    .recipe-palette {
        position: static;
        height: auto;
        min-height: 0;
        max-height: min(36rem, calc(800px - 5rem));
    }

    .recipe-generator-header {
        flex-direction: column;
    }

    .recipe-header-actions {
        width: 100%;
    }

    .recipe-version-control,
    .recipe-version-control :deep([data-combobox]) {
        width: 100%;
    }

    .recipe-header-actions > :deep([data-button]) {
        width: 100%;
        justify-content: center;
    }
}

@container recipe-generator-page (max-width: 62rem) {
    .recipe-workbench {
        grid-template-columns: minmax(0, 1fr);
    }

    .recipe-sidebar {
        position: static;
        height: auto;
        min-height: 0;
        max-height: none;
    }

    .recipe-palette {
        position: static;
        height: auto;
        min-height: 0;
        max-height: min(36rem, calc(800px - 5rem));
    }
}

@container recipe-palette (max-width: 16rem) {
    :deep(.recipe-palette-grid) {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}

@media (max-width: 40rem) {
    :deep(.recipe-palette-grid) {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}
</style>
