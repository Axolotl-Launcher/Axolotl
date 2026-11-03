import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

const thumbnailComposable = readFileSync(
    new URL('../../../composables/use-image-thumbnail.ts', import.meta.url),
    'utf8',
)
const card = readFileSync(new URL('./card.vue', import.meta.url), 'utf8')
const screenshotsPage = readFileSync(new URL('./index.vue', import.meta.url), 'utf8')

it('screenshot cards request bounded backend thumbnails instead of original files', () => {
    expect(thumbnailComposable).toMatch(/plugin:files\|screenshot_thumbnail/)
    expect(thumbnailComposable).toMatch(/MAX_CONCURRENT_THUMBNAILS = 4/)
    expect(card).toMatch(/filePath: `screenshots\/\$\{props\.screenshot\.file_name\}`/)
    expect(card).toMatch(/const thumbnail = useImageThumbnail\(/)
})

it('full screenshot URLs remain reserved for the image viewer', () => {
    expect(screenshotsPage).toMatch(/src: screenshot\.url/)
    expect(card).not.toMatch(/:src="screenshot\.url"/)
})

it('screenshot interaction states use the configured brand color', () => {
    expect(card).toMatch(/!border-brand brightness-110/)
    expect(card).toMatch(/rounded-full bg-brand/)
    expect(card).not.toMatch(/!border-contrast brightness-110/)
    expect(card).not.toMatch(/rounded-full bg-contrast/)
})
