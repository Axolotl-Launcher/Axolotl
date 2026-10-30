/**
 * Whether a finished plan may still navigate from the Select step. The run
 * itself is already known to be current; this only checks the page.
 */
export function isCurrentUpgradeSelectPlanning(
    disposed: boolean,
    routeName: unknown,
    routeInstanceId: unknown,
    instanceId: string,
): boolean {
    return !disposed && routeName === 'InstanceUpgrade' && routeInstanceId === instanceId
}
