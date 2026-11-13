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
 * Asks how each instance should come over, then queues an import job for each.
 * Resolves once every job is queued, with how many imports were started and a
 * promise for the jobs finishing, which can take minutes.
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

    // Launchers can hold same-named instances, so a choice is matched by its path as well.
    const choiceFor = (entry: LauncherImportEntry) =>
        choices.find(
            (choice) =>
                choice.instanceName === entry.instanceName &&
                (choice.instancePath === undefined || choice.instancePath === entry.instancePath),
        )
    // Every job is queued before this returns, so closing the launcher while the first one
    // copies cannot drop the rest; only their completion is left to `finished`.
    const jobs: string[] = []
    for (const entry of entries) {
        const choice = choiceFor(entry)
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
            jobs.push(job.job_id)
        } catch (error) {
            onError(error)
        }
    }
    const finished = (async () => {
        for (const jobId of jobs) {
            try {
                await wait_for_install_job(jobId)
            } catch (error) {
                onError(error)
            }
        }
    })()
    return { confirmed: jobs.length, finished }
}
