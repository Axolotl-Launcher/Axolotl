import type { DisplayStatus, ServerWorld, WorldWithInstance } from '@/helpers/worlds'

import recorded from '../data/remote-servers.json'
import { byId } from './instances'
import { desktopOnly } from './settings'
import type { Handlers } from './types'

const icons = import.meta.glob<string>('../assets/servers/*.png', {
    eager: true,
    query: '?url',
    import: 'default',
})

const servers: (ServerWorld & WorldWithInstance)[] = recorded.map((entry, index) => ({
    instance_id: entry.instance_id,
    name: entry.name,
    address: entry.address,
    index,
    type: 'server',
    display_status: 'favorite',
    pack_status: 'prompt',
    icon: entry.icon ? icons[`../assets/servers/${entry.icon}`] : undefined,
}))

let pinnedOrder = servers.map((entry) => entry.address)

export function isRecordedRemoteServer(address: string) {
    return servers.some((entry) => entry.address === address)
}

export function pinRemoteServers(addresses: string[]) {
    pinnedOrder = addresses.filter(isRecordedRemoteServer)
    for (const entry of servers)
        entry.display_status = pinnedOrder.includes(entry.address) ? 'favorite' : 'normal'
}

export const worldHandlers: Handlers = {
    'plugin:worlds|get_favorite_worlds': () =>
        servers
            .filter((entry) => entry.display_status === 'favorite' && byId(entry.instance_id))
            .sort((a, b) => pinnedOrder.indexOf(a.address) - pinnedOrder.indexOf(b.address)),
    'plugin:worlds|get_recent_worlds': () => [],
    'plugin:worlds|get_instance_worlds': ({ instanceId }) =>
        servers.filter((entry) => entry.instance_id === instanceId && byId(entry.instance_id)),
    'plugin:worlds|get_server_status': ({ address }) => {
        const entry = recorded.find((entry) => entry.address === address)
        if (!entry) throw desktopOnly()
        return {
            ...entry.status,
            favicon: entry.icon ? icons[`../assets/servers/${entry.icon}`] : undefined,
        }
    },
    'plugin:worlds|set_world_display_status': ({ instance, worldType, worldId, displayStatus }) => {
        const entry = servers.find(
            (entry) => entry.instance_id === instance && entry.address === worldId,
        )
        if (
            worldType !== 'server' ||
            !entry ||
            !['normal', 'hidden', 'favorite'].includes(String(displayStatus))
        )
            throw desktopOnly()
        entry.display_status = displayStatus as DisplayStatus
        if (displayStatus === 'favorite' && !pinnedOrder.includes(entry.address))
            pinnedOrder.push(entry.address)
    },
    'plugin:worlds|start_join_server': () => {
        throw desktopOnly()
    },
}
