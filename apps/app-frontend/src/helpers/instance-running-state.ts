export type InstanceProcessEvent = { instance_id: string; event: string }

export function updateRunningInstanceIds(
    current: ReadonlySet<string>,
    event: InstanceProcessEvent,
): Set<string> {
    const next = new Set(current)
    if (event.event === 'launched') next.add(event.instance_id)
    if (event.event === 'finished') next.delete(event.instance_id)
    return next
}
