import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { onScopeDispose, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { parseEula, setEulaAccepted } from '../../../lib/server-core.ts'
import { injectDownloadManager } from '../app-helpers.ts'
import { serverEventListener, servers as serversApi } from '../backend.ts'
import type EulaModal from '../servers/EulaModal.vue'
import { getServerInstallStrategy, runServerInstall } from './server-install.ts'
import { resumeModpackInstall } from './useServerInstalls.ts'
import { type ServerView, setServerExitReasonHandler, useServers } from './useServers.ts'

/**
 * Shared "start with EULA gate" flow used by the servers overview and the
 * server detail page. Bind the returned `eulaModal` ref to an `<EulaModal>`
 * rendered with `ref="eulaModal"` in the component template.
 */
export function useServerLifecycle() {
    const { startServer } = useServers()
    const { handleError } = injectNotificationManager()

    // [SERVER-DOWNLOAD-BRIDGE] Capture the download manager once during Vue
    // setup context.  See the note in `startModpackServerInstall` for why
    // this must be done here and not later.
    let downloadManager: ReturnType<typeof injectDownloadManager> | null = null
    try {
        downloadManager = injectDownloadManager()
    } catch {
        // Not inside a provider tree — server downloads will not appear in sidebar.
    }

    const eulaModal = useTemplateRef<ComponentExposed<typeof EulaModal>>('eulaModal')
    const eulaText = ref('')
    let pendingId = ''

    // Listen for real-time EULA prompt detection from the Rust backend
    let unregisterEulaListener: (() => void) | null = null
    serverEventListener((serverId, payload) => {
        if (payload.event === 'eula_required' && payload.server_id === pendingId) {
            eulaText.value = payload.eula_text
            eulaModal.value?.show()
        }
    }).then((unregister) => {
        unregisterEulaListener = unregister
    })
    onScopeDispose(() => {
        unregisterEulaListener?.()
    })

    /**
     * Starts the server, gating on the EULA file:
     * - eula.txt missing → code-create it with `eula=false`, then show the modal
     * - eula.txt present and `eula=false` → show the modal
     * - eula.txt present and `eula=true` → start directly
     */
    async function tryStartServer(server: ServerView) {
        const accepted = await ensureEula(server.id)
        if (accepted) {
            await launchServer(server.id)
        } else {
            pendingId = server.id
            eulaModal.value?.show()
        }
    }

    /**
     * Reads the server's eula.txt. When absent, writes a code-created
     * `eula=false` file so the acceptance prompt can be shown without booting
     * the server. Returns whether the EULA is already accepted.
     */
    async function ensureEula(serverId: string): Promise<boolean> {
        let text: string
        try {
            text = await serversApi.readFile(serverId, 'eula.txt')
        } catch {
            // eula.txt doesn't exist yet — code-create it with eula=false so the
            // manual start gate can offer the prompt without starting the jar.
            text = setEulaAccepted('', false)
            await serversApi.writeFile(serverId, 'eula.txt', text).catch(() => {})
        }
        const doc = parseEula(text)
        if (doc.accepted) return true
        eulaText.value = text
        return false
    }

    /**
     * Starts the server and, when the start itself fails over an unaccepted
     * EULA, falls back to the confirmation dialog instead of just the error.
     */
    async function launchServer(serverId: string) {
        const started = await startServer(serverId)
        if (started) return
        try {
            const text = await serversApi.readFile(serverId, 'eula.txt')
            if (parseEula(text).accepted) return
            eulaText.value = text
            pendingId = serverId
            eulaModal.value?.show()
        } catch {
            // Start failed for a non-EULA reason; that error was already surfaced.
        }
    }

    async function acceptEula() {
        const id = pendingId
        if (!id) return
        try {
            const updated = setEulaAccepted(eulaText.value, true)
            await serversApi.writeFile(id, 'eula.txt', updated)
            pendingId = ''
            eulaModal.value?.hide()
            // Start the server after accepting EULA
            await startServer(id)
        } catch (error) {
            console.error(error)
        }
    }

    function declineEula() {
        pendingId = ''
        eulaModal.value?.hide()
    }

    /**
     * Offers the EULA dialog after the server exited on its own over an
     * unaccepted EULA (detected from the process's final output). Accepting
     * writes `eula.txt` and restarts, matching the pre-start gate.
     */
    async function offerEulaAfterExit(serverId: string) {
        try {
            const text = await serversApi.readFile(serverId, 'eula.txt')
            if (parseEula(text).accepted) return
            eulaText.value = text
            pendingId = serverId
            eulaModal.value?.show()
        } catch {
            // No eula.txt to show; the exit stays unexplained.
        }
    }

    const unregisterExitReasonHandler = setServerExitReasonHandler((serverId, reason) => {
        if (reason === 'eula') void offerEulaAfterExit(serverId)
    })
    onScopeDispose(unregisterExitReasonHandler)

    /** Resumes an interrupted or failed install using its recorded server source. */
    async function resumeInstall(server: ServerView) {
        try {
            // [SERVER-DOWNLOAD-BRIDGE] Pass the download manager captured
            // during setup so the synthetic job appears in sidebar.
            if (server.modpack) {
                await resumeModpackInstall(server, downloadManager)
            } else {
                await runServerInstall({
                    serverId: server.id,
                    name: server.name,
                    inputs: {
                        gameVersion: server.gameVersion,
                        loaderVersion: server.loaderVersion,
                        javaPath: server.javaPath,
                        memoryMb: server.memoryMb,
                    },
                    strategy: getServerInstallStrategy(server.serverType),
                    downloadManager,
                })
            }
        } catch (error) {
            handleError(error)
        }
    }

    return {
        eulaModal,
        eulaText,
        tryStartServer,
        acceptEula,
        declineEula,
        resumeInstall,
        offerEulaAfterExit,
    }
}
