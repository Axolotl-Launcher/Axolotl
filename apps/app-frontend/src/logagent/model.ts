import {
    BaseChatModel,
    type BaseChatModelCallOptions,
    type BindToolsInput,
} from '@langchain/core/language_models/chat_models'
import { AIMessage, type BaseMessage, ToolMessage } from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { convertToOpenAITool } from '@langchain/core/utils/function_calling'

import type { AgentGateway, ModelMessage } from './types.ts'

interface ModelOptions extends BaseChatModelCallOptions {
    tools?: Record<string, unknown>[]
}

function serializeMessage(message: BaseMessage): ModelMessage {
    const type = message.type
    return {
        role: type === 'human' ? 'user' : type === 'ai' ? 'assistant' : type,
        content:
            typeof message.content === 'string'
                ? message.content
                : message.content
                      .filter((block) => block.type === 'text')
                      .map((block) => String(block.text ?? ''))
                      .join('\n'),
        tool_calls: AIMessage.isInstance(message)
            ? message.tool_calls?.map((call) => ({
                  id: call.id ?? '',
                  name: call.name,
                  args: call.args,
              }))
            : undefined,
        tool_call_id: ToolMessage.isInstance(message) ? message.tool_call_id : undefined,
        opaque: message.additional_kwargs.logagent_opaque,
    }
}

/** Keeps credentials and transport in Rust while preserving opaque provider state. */
export class LogAgentModel extends BaseChatModel<ModelOptions> {
    modelName = 'axolotl-logagent:diagnostics'
    private gateway: AgentGateway

    constructor(gateway: AgentGateway) {
        super({ callbacks: [], disableStreaming: true })
        this.gateway = gateway
    }

    _llmType() {
        return 'axolotl-logagent'
    }

    get callKeys() {
        return [...super.callKeys, 'tools']
    }

    bindTools(tools: BindToolsInput[]) {
        return this.withConfig({ tools: tools.map((tool) => convertToOpenAITool(tool)) })
    }

    async _generate(
        messages: BaseMessage[],
        options: this['ParsedCallOptions'],
    ): Promise<ChatResult> {
        options.signal?.throwIfAborted()
        const result = await this.gateway.modelTurn({
            messages: messages.map(serializeMessage),
            tools: options.tools ?? [],
        })
        options.signal?.throwIfAborted()
        const message = new AIMessage({
            content: result.content,
            tool_calls: result.tool_calls?.map((call) => ({ ...call, type: 'tool_call' as const })),
            additional_kwargs: { logagent_opaque: result.opaque },
        })
        return { generations: [{ text: result.content, message }] }
    }
}
