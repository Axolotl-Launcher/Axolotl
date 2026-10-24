export function isCurrentUpgradeSelectPlanning(
    disposed: boolean,
    generation: number,
    currentGeneration: number,
    routeName: unknown,
    routeInstanceId: unknown,
    instanceId: string,
): boolean {
    return (
        !disposed &&
        generation === currentGeneration &&
        routeName === 'InstanceUpgradeSelect' &&
        routeInstanceId === instanceId
    )
}
