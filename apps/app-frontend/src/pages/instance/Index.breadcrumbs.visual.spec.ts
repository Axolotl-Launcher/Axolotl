import { expect, it } from 'vitest'

import indexSource from './Index.vue?raw'

/**
 * Mounting the instance page needs its whole provider and child component rig,
 * so this pins the one thing that matters: the breadcrumb is rebuilt from what
 * it shows, not only when another instance opens.
 */
it('rebuilds the breadcrumb when the instance is renamed or gets a new icon', () => {
    const source = indexSource.replaceAll('\r\n', '\n')
    const watcher = source.slice(source.indexOf('updateBreadcrumbs()\n\nwatch('))
    const watched = watcher.slice(0, watcher.indexOf('=> {'))

    for (const field of ['id', 'name', 'icon_path', 'loader']) {
        expect(watched).toContain(`instance.value?.${field}`)
    }
})
