import { onScopeDispose, type Ref, shallowRef, watch } from 'vue'

import { useTranslationToggle } from '@/composables/useTranslationToggle'
import { type TranslatableHit, translateSearchDescriptions } from '@/helpers/translation'

export function useSearchResultTranslation<
    T extends TranslatableHit,
    S extends TranslatableHit,
>(options: {
    projectHits: Ref<T[]>
    serverHits: Ref<S[]>
    projectType: Ref<string>
    locale: Ref<string>
    onError: (error: unknown) => void
    translate?: typeof translateSearchDescriptions
}) {
    const control = useTranslationToggle()
    const originalProjects = shallowRef<T[]>([])
    const originalServers = shallowRef<S[]>([])
    const execute = options.translate ?? translateSearchDescriptions
    let applying = false

    async function apply<H extends TranslatableHit>(
        hits: Ref<H[]>,
        original: Ref<H[]>,
        version: number,
        force: boolean,
        server: boolean,
    ) {
        const snapshot = original.value
        if (!snapshot.length) return
        const translated = await execute(snapshot, options.locale.value, force, server)
        if (control.isStale(version)) return
        applying = true
        try {
            hits.value = translated
            control.translationActive.value = translated !== snapshot
        } finally {
            applying = false
        }
    }

    async function translate(force: boolean, version: number) {
        try {
            if (options.projectType.value === 'server')
                await apply(options.serverHits, originalServers, version, force, true)
            else await apply(options.projectHits, originalProjects, version, force, false)
        } catch (error) {
            if (!control.isStale(version)) options.onError(error)
        } finally {
            control.done(version)
        }
    }

    watch(
        [options.projectHits, options.serverHits, options.projectType, options.locale],
        ([projects, servers], [previousProjects, previousServers]) => {
            if (applying) return
            if (projects !== previousProjects) originalProjects.value = projects
            if (servers !== previousServers) originalServers.value = servers
            const version = control.cancel()
            void translate(false, version)
        },
        { flush: 'sync', immediate: true },
    )
    onScopeDispose(control.cancel)

    function toggle() {
        control.toggle(
            () => {
                applying = true
                try {
                    if (options.projectType.value === 'server')
                        options.serverHits.value = originalServers.value
                    else options.projectHits.value = originalProjects.value
                } finally {
                    applying = false
                }
            },
            () => void translate(true, control.start()),
        )
    }
    return {
        translationActive: control.translationActive,
        translationLoading: control.translationLoading,
        toggle,
    }
}
