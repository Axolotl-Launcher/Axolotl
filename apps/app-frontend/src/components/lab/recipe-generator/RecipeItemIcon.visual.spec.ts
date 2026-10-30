import '@modrinth/ui/directives/tooltip-styles.ts'

import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { afterEach, expect, it } from 'vitest'
import { page, userEvent } from 'vitest/browser'
import { createApp, defineComponent, h, type VNode, withDirectives } from 'vue'

import type { SlotDisplay } from '@/lab/recipe-generator/display'
import type { TextureAtlas } from '@/lab/shared/minecraft/resources'

import RecipeItemIcon from './RecipeItemIcon.vue'

/**
 * The icon renders a texture, not a description.
 *
 * Two `v-tooltip` triggers on one pointer path -- a control and the icon inside
 * it -- each acquire their own surface, so hovering the icon painted the item
 * label on top of the control's hint. Measured before the fix: both bubbles
 * were in the DOM at once. The icon therefore carries no tooltip of its own;
 * a control that has nothing else to say may still bind one to the icon.
 */

const ATLAS: TextureAtlas = { url: '', layout: {} }
const STONE: SlotDisplay = { label: 'Stone', texture: null, isTag: false }

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn())
    document.querySelectorAll('.tooltip-positioner').forEach((node) => node.remove())
})

function mount(render: () => VNode) {
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(defineComponent({ render }))
    // `main.js` registers the directive globally, so a component template that
    // uses `v-tooltip` itself only resolves when the harness does the same.
    app.directive('tooltip', tooltipDirective)
    app.mount(host)
    cleanup.push(() => {
        app.unmount()
        host.remove()
    })
    return host
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

const surfaces = () =>
    [...document.querySelectorAll('.tooltip-popper')].map((element) => element.textContent?.trim())

it('leaves the surrounding control as the only tooltip on the pointer path', async () => {
    const host = mount(() =>
        withDirectives(
            h(
                'button',
                {
                    style: 'position:absolute;top:120px;left:120px;width:240px;height:100px',
                },
                [h(RecipeItemIcon, { display: STONE, atlas: ATLAS })],
            ),
            [[tooltipDirective, 'Add item: Stone']],
        ),
    )

    await userEvent.hover(page.elementLocator(host.querySelector('button > div') as HTMLElement))
    await sleep(400)

    expect(surfaces()).toEqual(['Add item: Stone'])
})

it('shows the label a caller binds to the icon itself', async () => {
    const host = mount(() =>
        withDirectives(
            h(RecipeItemIcon, {
                display: STONE,
                atlas: ATLAS,
                style: 'position:absolute;top:120px;left:120px',
            }),
            [[tooltipDirective, 'Stone']],
        ),
    )

    await userEvent.hover(page.elementLocator(host.querySelector('div') as HTMLElement))
    await sleep(400)

    expect(surfaces()).toEqual(['Stone'])
})
