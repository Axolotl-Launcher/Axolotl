import { getSlotDisplay } from './display.ts'
import { getCraftingGridValues } from './recipe-engine.ts'
import type { TextureAtlas } from './resources.ts'
import type { RecipeSlotContext, RecipeState } from './types.ts'

const imageCache = new Map<string, Promise<HTMLImageElement>>()

function loadImage(url: string): Promise<HTMLImageElement> {
    const cached = imageCache.get(url)
    if (cached) return cached
    const promise = new Promise<HTMLImageElement>((resolve, reject) => {
        const image = new Image()
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error(`Unable to load texture: ${url}`))
        image.src = url
    })
    imageCache.set(url, promise)
    return promise
}

async function drawSlot(
    canvas: HTMLCanvasElement,
    x: number,
    y: number,
    value: ReturnType<typeof getSlotDisplay>,
    atlas: TextureAtlas,
    image: HTMLImageElement,
): Promise<void> {
    const context = canvas.getContext('2d')
    if (!context) return
    const size = 32
    context.fillStyle = 'rgba(139,139,139,1)'
    context.fillRect(x, y, size, size)
    context.strokeStyle = 'rgba(55,55,55,1)'
    context.lineWidth = 1
    context.strokeRect(x + 0.5, y + 0.5, size - 1, size - 1)
    if (value?.texture && atlas.layout[value.texture]) {
        const [ux, uy, uw, uh] = atlas.layout[value.texture]
        context.drawImage(image, ux, uy, uw, uh, x + 1, y + 1, 30, 30)
    } else if (value?.texture) {
        try {
            const customImage = await loadImage(value.texture)
            context.drawImage(customImage, x + 1, y + 1, 30, 30)
        } catch {
            // Custom textures may be unreachable; keep the empty slot.
        }
    }
    if (value?.count && value.count > 1) {
        context.fillStyle = 'rgba(255,255,255,1)'
        context.font = 'bold 12px sans-serif'
        context.textAlign = 'right'
        context.textBaseline = 'bottom'
        context.shadowColor = 'rgba(0,0,0,1)'
        context.shadowOffsetX = 1
        context.shadowOffsetY = 1
        context.fillText(String(value.count), x + size - 2, y + size - 2)
        context.shadowColor = 'transparent'
    }
}

function canvasSize(recipe: RecipeState): { width: number; height: number } {
    if (recipe.recipeType === 'crafting') {
        return { width: 220, height: 130 }
    }
    if (
        recipe.recipeType === 'smelting' ||
        recipe.recipeType === 'blasting' ||
        recipe.recipeType === 'smoking' ||
        recipe.recipeType === 'campfire_cooking'
    ) {
        return { width: 200, height: 90 }
    }
    if (recipe.recipeType === 'stonecutter') {
        return { width: 220, height: 110 }
    }
    return { width: 300, height: 120 }
}

async function drawPreview(
    canvas: HTMLCanvasElement,
    recipe: RecipeState,
    ctx: RecipeSlotContext,
    atlas: TextureAtlas,
    image: HTMLImageElement,
): Promise<void> {
    const context = canvas.getContext('2d')
    if (!context) return
    context.clearRect(0, 0, canvas.width, canvas.height)
    context.fillStyle = 'rgba(198,198,198,1)'
    context.fillRect(0, 0, canvas.width, canvas.height)

    if (recipe.recipeType === 'crafting') {
        const grid = getCraftingGridValues(recipe)
        for (const [index, value] of grid.entries()) {
            const x = 10 + (index % 3) * 40
            const y = 14 + Math.floor(index / 3) * 40
            await drawSlot(canvas, x, y, getSlotDisplay(value, ctx), atlas, image)
        }
        await drawSlot(
            canvas,
            150,
            54,
            getSlotDisplay(recipe.slots['crafting.result'], ctx),
            atlas,
            image,
        )
        return
    }

    if (
        recipe.recipeType === 'smelting' ||
        recipe.recipeType === 'blasting' ||
        recipe.recipeType === 'smoking' ||
        recipe.recipeType === 'campfire_cooking'
    ) {
        await drawSlot(
            canvas,
            20,
            30,
            getSlotDisplay(recipe.slots['cooking.ingredient'], ctx),
            atlas,
            image,
        )
        await drawSlot(
            canvas,
            140,
            30,
            getSlotDisplay(recipe.slots['cooking.result'], ctx),
            atlas,
            image,
        )
        return
    }

    if (recipe.recipeType === 'brewing') {
        for (const [index, slot] of (
            ['brewing.input', 'brewing.reagent', 'brewing.result'] as const
        ).entries())
            await drawSlot(
                canvas,
                15 + index * 105,
                45,
                getSlotDisplay(recipe.slots[slot], ctx),
                atlas,
                image,
            )
        return
    }
    if (recipe.recipeType === 'stonecutter') {
        await drawSlot(
            canvas,
            20,
            40,
            getSlotDisplay(recipe.slots['stonecutter.ingredient'], ctx),
            atlas,
            image,
        )
        await drawSlot(
            canvas,
            160,
            40,
            getSlotDisplay(recipe.slots['stonecutter.result'], ctx),
            atlas,
            image,
        )
        return
    }

    const smithingSlots = [
        ['smithing.template', 10] as const,
        ['smithing.base', 62] as const,
        ['smithing.addition', 114] as const,
        ['smithing.result', 240] as const,
    ]
    for (const [slot, x] of smithingSlots) {
        await drawSlot(canvas, x, 44, getSlotDisplay(recipe.slots[slot], ctx), atlas, image)
    }
}

export async function createRecipePreviewPngBlob(
    recipe: RecipeState,
    ctx: RecipeSlotContext,
    atlas: TextureAtlas,
): Promise<Blob> {
    const image = await loadImage(atlas.url)
    const { width, height } = canvasSize(recipe)
    const canvas = document.createElement('canvas')
    canvas.width = width * 2
    canvas.height = height * 2
    const context = canvas.getContext('2d')
    if (!context) throw new Error('Unable to create a canvas context')
    context.scale(2, 2)
    await drawPreview(canvas, recipe, ctx, atlas, image)
    return await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob((result) => {
            if (result) resolve(result)
            else reject(new Error('Unable to encode preview PNG'))
        }, 'image/png')
    })
}

export async function copyRecipePreviewToClipboard(
    recipe: RecipeState,
    ctx: RecipeSlotContext,
    atlas: TextureAtlas,
): Promise<void> {
    const blob = await createRecipePreviewPngBlob(recipe, ctx, atlas)
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])
}
