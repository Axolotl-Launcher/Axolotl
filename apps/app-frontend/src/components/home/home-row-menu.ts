import {
    ClipboardCopyIcon,
    ExternalIcon,
    EyeIcon,
    FolderOpenIcon,
    PinIcon,
    PlayIcon,
    UpdatedIcon,
} from '@modrinth/assets'
import {
    commonMessages,
    defineMessages,
    injectNotificationManager,
    type MenuAction,
    useVIntl,
} from '@modrinth/ui'
import { defineComponent, h } from 'vue'
import { useRouter } from 'vue-router'

import type { GameInstance } from '@/helpers/types'
import { copyToClipboard, createInstanceShortcut, showInstanceInFolder } from '@/helpers/utils'
import {
    getWorldIdentifier,
    set_world_display_status,
    showWorldInFolder,
    type WorldWithInstance,
} from '@/helpers/worlds'

/** The pin, turned the way the rest of the app shows "unpin". */
export const UnpinIcon = defineComponent({
    name: 'UnpinIcon',
    setup: () => () => h(PinIcon, { class: 'rotate-45', 'aria-hidden': 'true' }),
})

const messages = defineMessages({
    playInstance: { id: 'instance.worlds.play_instance', defaultMessage: 'Play instance' },
    viewInstance: { id: 'instance.worlds.view_instance', defaultMessage: 'View instance' },
    copyAddress: { id: 'instance.worlds.copy_address', defaultMessage: 'Copy address' },
    createShortcut: { id: 'instance.worlds.create_shortcut', defaultMessage: 'Create shortcut' },
    pinToHome: { id: 'app.instances.pin-to-home', defaultMessage: 'Pin to Home' },
    unpinFromHome: { id: 'app.instances.unpin-from-home', defaultMessage: 'Unpin from Home' },
    shortcutCreated: { id: 'app.home.row.shortcut-created', defaultMessage: 'Shortcut created' },
})

/**
 * The "more" menus of Home rows. They carry what the shared world and instance
 * rows offered before the Home redesign, in the same sections and order, so a
 * row on Home can do what the same row can do elsewhere.
 */
export function useHomeRowMenu() {
    const { formatMessage } = useVIntl()
    const { addNotification, handleError } = injectNotificationManager()
    const router = useRouter()

    function viewInstance(instance: GameInstance): MenuAction {
        return {
            id: 'open-instance',
            section: 'navigate',
            icon: EyeIcon,
            label: formatMessage(messages.viewInstance),
            action: () => router.push(`/instance/${encodeURIComponent(instance.id)}`),
        }
    }

    function homePin(pinned: boolean, update: () => Promise<unknown>): MenuAction {
        return {
            id: pinned ? 'unpin-home' : 'pin-home',
            section: 'pin',
            icon: pinned ? UnpinIcon : PinIcon,
            label: formatMessage(pinned ? messages.unpinFromHome : messages.pinToHome),
            action: () => void update().catch(handleError),
        }
    }

    async function createShortcut(
        name: string,
        instanceId: string,
        options?: { server?: string; singleplayerWorld?: string },
    ) {
        try {
            const path = await createInstanceShortcut(name, instanceId, options)
            if (path) {
                addNotification({ type: 'success', title: formatMessage(messages.shortcutCreated) })
            }
        } catch (error) {
            handleError(error as Error)
        }
    }

    /** An instance row: open it, open its folder, and (un)pin it when the widget can. */
    function instanceMenu(
        instance: GameInstance,
        pin?: { pinned: boolean; update: () => Promise<unknown> },
    ): MenuAction[] {
        return [
            viewInstance(instance),
            {
                id: 'open-folder',
                section: 'navigate',
                icon: FolderOpenIcon,
                label: formatMessage(commonMessages.openFolderButton),
                action: () => void showInstanceInFolder(instance.id).catch(handleError),
            },
            ...(pin ? [homePin(pin.pinned, pin.update)] : []),
        ]
    }

    /** A world or server row, as the instance's worlds page offers it. */
    function worldMenu(
        world: WorldWithInstance,
        instance: GameInstance,
        context: {
            instanceRunning: boolean
            playInstance: () => unknown
            refreshServer?: () => unknown
            /** Called after the Home pin changed, to reload the lists. */
            pinChanged: () => Promise<unknown>
        },
    ): MenuAction[] {
        const pinned = world.display_status === 'favorite'
        const options: MenuAction[] = [
            {
                id: 'play-instance',
                section: 'primary',
                icon: PlayIcon,
                label: formatMessage(messages.playInstance),
                disabled: context.instanceRunning,
                action: () => void context.playInstance(),
            },
            viewInstance(instance),
        ]

        if (world.type === 'server') {
            options.push({
                id: 'copy-address',
                section: 'navigate',
                icon: ClipboardCopyIcon,
                label: formatMessage(messages.copyAddress),
                action: () => void copyToClipboard(world.address).catch(handleError),
            })
            if (context.refreshServer) {
                const refresh = context.refreshServer
                options.push({
                    id: 'refresh',
                    section: 'manage',
                    icon: UpdatedIcon,
                    label: formatMessage(commonMessages.refreshButton),
                    action: () => void refresh(),
                })
            }
        } else {
            options.push({
                id: 'open-folder',
                section: 'navigate',
                icon: FolderOpenIcon,
                label: formatMessage(commonMessages.openFolderButton),
                action: () => void showWorldInFolder(instance.id, world.path).catch(handleError),
            })
        }

        options.push(
            homePin(pinned, async () => {
                await set_world_display_status(
                    instance.id,
                    world.type,
                    getWorldIdentifier(world),
                    pinned ? 'normal' : 'favorite',
                )
                await context.pinChanged()
            }),
            {
                id: 'create-shortcut',
                section: 'navigate',
                icon: ExternalIcon,
                label: formatMessage(messages.createShortcut),
                action: () =>
                    void createShortcut(
                        world.name,
                        instance.id,
                        world.type === 'server'
                            ? { server: world.address }
                            : { singleplayerWorld: world.path },
                    ),
            },
        )
        return options
    }

    return { instanceMenu, worldMenu, homePin }
}
