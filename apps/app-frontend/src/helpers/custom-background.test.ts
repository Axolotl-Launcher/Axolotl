import { expect, it } from 'vitest'

import {
    classifyCustomBackground,
    customBackgroundMimeType,
    extensionFromPath,
    isCroppableCustomBackground,
    isCustomBackgroundImagePath,
    isCustomBackgroundWithinByteLimit,
    MAX_CUSTOM_BACKGROUND_BYTES,
} from './custom-background.ts'

const stillPng = new Uint8Array([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44,
])

const animatedPng = new Uint8Array([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0, 0x61, 0x63, 0x54, 0x4c, 0, 0, 0, 0,
])

const stillWebp = new Uint8Array([
    0x52, 0x49, 0x46, 0x46, 0x04, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x20,
])

const animatedWebp = new Uint8Array([
    0x52, 0x49, 0x46, 0x46, 0x04, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0x41, 0x4e, 0x49, 0x4d, 0, 0, 0,
    0,
])

it('recognizes picker and future launcher background extensions', () => {
    expect(extensionFromPath(String.raw`C:\pictures\背景.PNG`)).toBe('png')
    expect(extensionFromPath('/pictures/photo.JPEG')).toBe('jpeg')
    expect(extensionFromPath('/pictures/photo.jpg')).toBe('jpg')
    expect(extensionFromPath('/pictures/wallpaper.webp')).toBe('webp')
    expect(extensionFromPath('/pictures/wallpaper.bmp')).toBe('bmp')
    expect(extensionFromPath('/pictures/loop.gif')).toBe('gif')
    expect(extensionFromPath('/pictures/photo.avif')).toBe('avif')
    expect(extensionFromPath('/pictures/clip.mp4')).toBe('mp4')

    expect(extensionFromPath('/pictures/README')).toBeNull()
    expect(isCustomBackgroundImagePath('/pictures/loop.gif')).toBe(true)
    expect(isCustomBackgroundImagePath('/pictures/wallpaper.png')).toBe(true)
})

it('maps extensions to their MIME types', () => {
    expect(customBackgroundMimeType('png')).toBe('image/png')
    expect(customBackgroundMimeType('jpeg')).toBe('image/jpeg')
    expect(customBackgroundMimeType('jpg')).toBe('image/jpeg')
    expect(customBackgroundMimeType('webp')).toBe('image/webp')
    expect(customBackgroundMimeType('bmp')).toBe('image/bmp')
    expect(customBackgroundMimeType('gif')).toBe('image/gif')
    expect(customBackgroundMimeType('avif')).toBe('image/avif')
    expect(customBackgroundMimeType('mp4')).toBe('video/mp4')
})

it('classifies still, animated, and future video backgrounds', () => {
    expect(classifyCustomBackground('png', stillPng)).toBe('static')
    expect(classifyCustomBackground('png', animatedPng)).toBe('animated-image')
    expect(classifyCustomBackground('webp', stillWebp)).toBe('static')
    expect(classifyCustomBackground('webp', animatedWebp)).toBe('animated-image')
    expect(classifyCustomBackground('gif', stillPng)).toBe('animated-image')
    expect(classifyCustomBackground('jpeg', stillPng)).toBe('static')
    expect(classifyCustomBackground('bmp', stillPng)).toBe('static')
    expect(classifyCustomBackground('avif', stillPng)).toBe('animated-avif')
    expect(classifyCustomBackground('mp4', stillPng)).toBe('video')
})

it('keeps the static crop predicate separate from animation support', () => {
    expect(isCroppableCustomBackground('image/gif', stillPng)).toBe(false)
    expect(isCroppableCustomBackground('image/avif', stillPng)).toBe(false)
    expect(isCroppableCustomBackground('image/svg+xml', stillPng)).toBe(false)
})

it('accepts still PNG and WebP but rejects animated containers', () => {
    expect(isCroppableCustomBackground('image/png', stillPng)).toBe(true)
    expect(isCroppableCustomBackground('image/webp', stillWebp)).toBe(true)

    expect(isCroppableCustomBackground('image/png', animatedPng)).toBe(false)
    expect(isCroppableCustomBackground('image/webp', animatedWebp)).toBe(false)
    expect(isCroppableCustomBackground('image/png', null)).toBe(false)
})

it('accepts JPEG and BMP without inspecting container frames', () => {
    expect(isCroppableCustomBackground('image/jpeg', new Uint8Array())).toBe(true)
    expect(isCroppableCustomBackground('image/bmp', new Uint8Array())).toBe(true)
})

it('rejects backgrounds above the stored byte limit', () => {
    expect(isCustomBackgroundWithinByteLimit(MAX_CUSTOM_BACKGROUND_BYTES)).toBe(true)
    expect(isCustomBackgroundWithinByteLimit(MAX_CUSTOM_BACKGROUND_BYTES - 1)).toBe(true)
    expect(isCustomBackgroundWithinByteLimit(MAX_CUSTOM_BACKGROUND_BYTES + 1)).toBe(false)
})
