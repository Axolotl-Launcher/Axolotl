import {
    AsyncLocalStorageProviderSingleton,
    MockAsyncLocalStorage,
} from '@langchain/core/singletons'
import { interrupt } from '@langchain/langgraph'

// Browsers have no Node AsyncLocalStorage. Bind only the synchronous interrupt
// call to the node's explicit config; never keep a global task across an await.
// Graphs can therefore overlap without borrowing another task's scratchpad.
class InterruptScope {
    private stack: unknown[] = []
    getStore() {
        return this.stack.at(-1)
    }
    run<T>(store: unknown, callback: () => T): T {
        this.stack.push(store)
        try {
            return callback()
        } finally {
            this.stack.pop()
        }
    }
    enterWith(store: unknown) {
        if (this.stack.length) this.stack[this.stack.length - 1] = store
    }
}

if (AsyncLocalStorageProviderSingleton.getInstance() instanceof MockAsyncLocalStorage)
    AsyncLocalStorageProviderSingleton.initializeGlobalInstance(new InterruptScope())

export function nodeInterrupt(
    value: unknown,
    configurable: Record<string, unknown> | undefined,
): unknown {
    if (!configurable) throw new Error('Approval has no graph node configuration')
    return AsyncLocalStorageProviderSingleton.runWithConfig({ configurable }, () =>
        interrupt(value),
    )
}
