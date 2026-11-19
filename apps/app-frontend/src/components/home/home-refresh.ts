export function homeWorldsChanged(event: string) {
    return [
        'created',
        'added',
        'removed',
        'edited',
        'world_updated',
        'server_joined',
        'servers_updated',
    ].includes(event)
}
