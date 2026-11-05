import { LauncherChatModel } from '@axolotl/agent-runtime/model'

import type { AgentGateway } from './types.ts'

/** Diagnostics retain their own tools and budgets; only model transport is shared. */
export class LogAgentModel extends LauncherChatModel {
    constructor(gateway: AgentGateway) {
        super(gateway, {
            modelName: 'axolotl-logagent:diagnostics',
            providerType: 'axolotl-logagent',
        })
    }
}
