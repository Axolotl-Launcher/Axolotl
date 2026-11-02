import type { RunnableConfig } from '@langchain/core/runnables'
import {
    BaseCheckpointSaver,
    type ChannelVersions,
    type Checkpoint,
    type CheckpointListOptions,
    type CheckpointMetadata,
    type CheckpointTuple,
    type PendingWrite,
    WRITES_IDX_MAP,
} from '@langchain/langgraph-checkpoint'

import type { RuntimeGateway, SerializedValue, StoredCheckpoint } from './protocol.ts'

function base64(bytes: Uint8Array) {
    const parts: string[] = []
    for (let i = 0; i < bytes.length; i += 16384)
        parts.push(String.fromCharCode(...bytes.subarray(i, i + 16384)))
    return btoa(parts.join(''))
}

function bytes(data: string) {
    return Uint8Array.from(atob(data), (character) => character.charCodeAt(0))
}

/** Durable LangGraph checkpoints, including pending writes. The host, not this
 * adapter, owns SQLite transactions and enforces attempt ownership and lifecycle. */
export class HostCheckpointer extends BaseCheckpointSaver {
    constructor(
        private attemptId: string,
        private gateway: Pick<RuntimeGateway, 'checkpoint'>,
    ) {
        super()
    }

    private scope(config: RunnableConfig) {
        if (config.configurable?.thread_id !== this.attemptId)
            throw new Error('Checkpoint belongs to another run attempt')
        const namespace = config.configurable?.checkpoint_ns ?? ''
        if (typeof namespace !== 'string' || namespace.length > 2048)
            throw new Error('Invalid checkpoint namespace')
        return namespace
    }

    private async pack(value: unknown): Promise<SerializedValue> {
        const [type, data] = await this.serde.dumpsTyped(value)
        return { type, data: base64(data) }
    }

    private unpack(value: SerializedValue) {
        return this.serde.loadsTyped(value.type, bytes(value.data))
    }

    private config(namespace: string, id: string): RunnableConfig {
        return {
            configurable: {
                thread_id: this.attemptId,
                checkpoint_ns: namespace,
                checkpoint_id: id,
            },
        }
    }

    private async tuple(namespace: string, record: StoredCheckpoint): Promise<CheckpointTuple> {
        return {
            config: this.config(namespace, record.id),
            checkpoint: (await this.unpack(record.checkpoint)) as Checkpoint,
            metadata: (await this.unpack(record.metadata)) as CheckpointMetadata,
            ...(record.parent_id ? { parentConfig: this.config(namespace, record.parent_id) } : {}),
            pendingWrites: await Promise.all(
                record.writes.map(
                    async (write) =>
                        [write.task_id, write.channel, await this.unpack(write.value)] as [
                            string,
                            string,
                            unknown,
                        ],
                ),
            ),
        }
    }

    async getTuple(config: RunnableConfig): Promise<CheckpointTuple | undefined> {
        const namespace = this.scope(config)
        const records = await this.gateway.checkpoint({
            operation: 'get',
            namespace,
            id: config.configurable?.checkpoint_id,
        })
        return records?.[0] ? this.tuple(namespace, records[0]) : undefined
    }

    async *list(
        config: RunnableConfig,
        options?: CheckpointListOptions,
    ): AsyncGenerator<CheckpointTuple> {
        const namespace = this.scope(config)
        // Filter before applying the requested limit. Each attempt has a bounded
        // graph budget; host pagination still prevents unbounded IPC responses.
        const records = await this.gateway.checkpoint({
            operation: 'list',
            namespace,
            id: config.configurable?.checkpoint_id,
            before: options?.before?.configurable?.checkpoint_id,
        })
        let left = options?.limit ?? Number.MAX_SAFE_INTEGER
        for (const record of records ?? []) {
            if (left <= 0) return
            const tuple = await this.tuple(namespace, record)
            if (
                options?.filter &&
                !Object.entries(options.filter).every(
                    ([key, value]) =>
                        (tuple.metadata as Record<string, unknown> | undefined)?.[key] === value,
                )
            )
                continue
            left--
            yield tuple
        }
    }

    async put(
        config: RunnableConfig,
        checkpoint: Checkpoint,
        metadata: CheckpointMetadata,
        _newVersions: ChannelVersions,
    ): Promise<RunnableConfig> {
        const namespace = this.scope(config)
        await this.gateway.checkpoint({
            operation: 'put',
            namespace,
            record: {
                id: checkpoint.id,
                checkpoint: await this.pack(checkpoint),
                metadata: await this.pack(metadata),
                parent_id: config.configurable?.checkpoint_id,
            },
        })
        return this.config(namespace, checkpoint.id)
    }

    async putWrites(config: RunnableConfig, writes: PendingWrite[], taskId: string) {
        const namespace = this.scope(config)
        const id = config.configurable?.checkpoint_id
        if (typeof id !== 'string') throw new Error('Missing checkpoint for pending writes')
        await this.gateway.checkpoint({
            operation: 'put_writes',
            namespace,
            id,
            writes: await Promise.all(
                writes.map(async ([channel, value], index) => ({
                    task_id: taskId,
                    index: WRITES_IDX_MAP[channel] ?? index,
                    channel,
                    value: await this.pack(value),
                })),
            ),
        })
    }

    async deleteThread(_threadId: string): Promise<void> {
        // Deletion is a UI-owned task operation; the worker cannot erase evidence.
        throw new Error('Only the Agent Window may delete a task')
    }
}
