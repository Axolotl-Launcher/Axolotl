import { describe, expect, it } from 'vitest'

import { THEME_PACK_ORIGIN, themePackAssetUrl } from './theme-packs'

describe('themePackAssetUrl', () => {
    it('escapes characters that could break out of a CSS url() string', () => {
        expect(themePackAssetUrl('demo', "assets/it's).css")).toBe(
            `${THEME_PACK_ORIGIN}/demo/assets/it%27s%29.css`,
        )
    })

    it('keeps nested asset paths readable', () => {
        expect(themePackAssetUrl('demo', 'assets/fonts/ui font.woff2')).toBe(
            `${THEME_PACK_ORIGIN}/demo/assets/fonts/ui%20font.woff2`,
        )
    })
})
