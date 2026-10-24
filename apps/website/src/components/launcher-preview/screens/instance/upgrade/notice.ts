export function shouldExpandUpgradeWarningsByDefault(count: number): boolean {
    return count > 0 && count <= 10
}
