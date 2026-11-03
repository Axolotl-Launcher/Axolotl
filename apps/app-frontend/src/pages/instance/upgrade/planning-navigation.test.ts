import { expect, it } from 'vitest'

import { isCurrentUpgradeSelectPlanning } from './planning-navigation.ts'

it('planner continuation navigates only while same Select request remains current', () => {
    expect(isCurrentUpgradeSelectPlanning(false, 'InstanceUpgrade', 'a', 'a')).toBe(true)
    expect(isCurrentUpgradeSelectPlanning(false, 'InstanceContent', 'a', 'a')).toBe(false)
    expect(isCurrentUpgradeSelectPlanning(false, 'InstanceUpgrade', 'b', 'a')).toBe(false)
    expect(isCurrentUpgradeSelectPlanning(true, 'InstanceUpgrade', 'a', 'a')).toBe(false)
})

it('pending planner completion does not navigate after route changes', async () => {
    let resolvePlanner!: () => void
    const planner = new Promise<void>((resolve) => {
        resolvePlanner = resolve
    })
    let routeName = 'InstanceUpgrade'
    let navigations = 0
    const continuation = planner.then(() => {
        if (isCurrentUpgradeSelectPlanning(false, routeName, 'a', 'a')) navigations += 1
    })

    routeName = 'InstanceContent'
    resolvePlanner()
    await continuation

    expect(navigations).toBe(0)
})

it('pending planner completion navigates once while Select remains current', async () => {
    let resolvePlanner!: () => void
    const planner = new Promise<void>((resolve) => {
        resolvePlanner = resolve
    })
    let navigations = 0
    const continuation = planner.then(() => {
        if (isCurrentUpgradeSelectPlanning(false, 'InstanceUpgrade', 'a', 'a')) {
            navigations += 1
        }
    })

    resolvePlanner()
    await continuation

    expect(navigations).toBe(1)
})
