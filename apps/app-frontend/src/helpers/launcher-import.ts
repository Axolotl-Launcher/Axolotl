import type { SymlinkMethodChoice, SymlinkMethodInstance } from '@modrinth/ui'

import { import_instance } from '@/helpers/import.js'
import { wait_for_install_job } from '@/helpers/install'

/** One instance another launcher keeps, as the import commands name it. */
export type LauncherImportEntry = {
    launcherType: string
    path: string
    instanceName: string
    instancePath: string
}

export type ChooseImportMethod = (options: {
    instances: SymlinkMethodInstance[]
}) => Promise<SymlinkMethodChoice[]>

/**
 * Asks how each instance should come over, then imports them one after another.
 * Resolves once the user has answered, with how many they confirmed and a
 * promise for the imports themselves, which can take minutes.
 */
export async function startLauncherImport(
    entries: readonly LauncherImportEntry[],
    chooseImportMethod: ChooseImportMethod,
    onError: (error: unknown) => void,
): Promise<{ confirmed: number; finished: Promise<void> }> {
    if (entries.length === 0) return { confirmed: 0, finished: Promise.resolve() }

    const choices = await chooseImportMethod({
        instances: entries.map((entry) => ({
            name: entry.instanceName,
            path: entry.instancePath,
            launcherType: entry.launcherType,
            basePath: entry.path,
        })),
    })
    if (choices.length === 0) return { confirmed: 0, finished: Promise.resolve() }

    const choiceByInstanceName = new Map(choices.map((choice) => [choice.instanceName, choice]))
    const finished = (async () => {
        for (const entry of entries) {
            const choice = choiceByInstanceName.get(entry.instanceName)
            try {
                const job = await import_instance(
                    entry.launcherType,
                    entry.path,
                    entry.instanceName,
                    false,
                    entry.instancePath,
                    choice?.gameVersion ?? undefined,
                    choice?.loader ?? undefined,
                    choice?.loaderVersion ?? undefined,
                    null,
                )
                await wait_for_install_job(job.job_id)
            } catch (error) {
                onError(error)
            }
        }
    })()
    return { confirmed: choices.length, finished }
}
