import {
    BaseChatModel,
    type BaseChatModelCallOptions,
    type BindToolsInput,
} from '@langchain/core/language_models/chat_models'
import {
    AIMessage,
    type BaseMessage,
    HumanMessage,
    SystemMessage,
    ToolMessage,
} from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { convertToOpenAITool } from '@langchain/core/utils/function_calling'

import type { ModelGateway, ModelImage, ModelMessage, ModelStreamEvent } from './protocol.ts'

interface ModelOptions extends BaseChatModelCallOptions {
    tools?: Record<string, unknown>[]
}

function image(url: unknown): ModelImage {
    if (typeof url !== 'string') throw new Error('Unsupported image; attach PNG, JPEG or WebP data')
    const match = url.match(/^data:(image\/(?:png|jpeg|webp));base64,([A-Za-z0-9+/]+={0,2})$/)
    if (!match || match[2].length > 7 * 1024 * 1024)
        throw new Error('Unsupported or oversized image; attach PNG, JPEG or WebP data')
    return { mime_type: match[1] as ModelImage['mime_type'], data: match[2] }
}

export function serializeMessage(message: BaseMessage): ModelMessage {
    const texts: string[] = []
    const images: ModelImage[] = []
    if (typeof message.content === 'string') texts.push(message.content)
    else
        for (const block of message.content) {
            if (block.type === 'text') texts.push(String(block.text ?? ''))
            else if (block.type === 'image_url') {
                const source = block.image_url
                images.push(
                    image(
                        typeof source === 'string'
                            ? source
                            : source && typeof source === 'object' && 'url' in source
                              ? source.url
                              : undefined,
                    ),
                )
            } else if (block.type === 'image' && block.source_type === 'base64') {
                images.push(image(`data:${block.mime_type};base64,${block.data}`))
            } else
                throw new Error(
                    `Unsupported model content: ${block.type}. Switch to a compatible model.`,
                )
        }
    return {
        role:
            message.type === 'human' ? 'user' : message.type === 'ai' ? 'assistant' : message.type,
        content: texts.join('\n'),
        ...(images.length ? { images } : {}),
        tool_calls: AIMessage.isInstance(message)
            ? message.tool_calls?.map((call) => ({
                  id: call.id ?? '',
                  name: call.name,
                  args: call.args,
              }))
            : undefined,
        tool_call_id: ToolMessage.isInstance(message) ? message.tool_call_id : undefined,
        tool_error: ToolMessage.isInstance(message) ? message.status === 'error' : undefined,
        opaque:
            message.additional_kwargs.axolotl_opaque ?? message.additional_kwargs.logagent_opaque,
    }
}

export function deserializeMessage(message: ModelMessage): BaseMessage {
    const content = message.images?.length
        ? [
              { type: 'text' as const, text: message.content },
              ...message.images.map((item) => ({
                  type: 'image_url' as const,
                  image_url: { url: `data:${item.mime_type};base64,${item.data}` },
              })),
          ]
        : message.content
    if (message.role === 'assistant')
        return new AIMessage({
            content,
            tool_calls: message.tool_calls?.map((call) => ({
                ...call,
                type: 'tool_call' as const,
            })),
            additional_kwargs: { axolotl_opaque: message.opaque },
        })
    if (message.role === 'tool')
        return new ToolMessage({
            content,
            tool_call_id: message.tool_call_id ?? '',
            status: message.tool_error ? 'error' : 'success',
        })
    if (message.role === 'user') return new HumanMessage({ content })
    if (message.role === 'system') return new SystemMessage({ content })
    throw new Error(`Unsupported model message role: ${message.role}`)
}

/** Shared model adapter: LangChain orchestration in JS, credentialed transport in Rust.
 * Rust streaming callbacks feed the UI without invoking a second model request. */
export class LauncherChatModel extends BaseChatModel<ModelOptions> {
    modelName: string
    private gateway: ModelGateway
    private onEvent?: (event: ModelStreamEvent) => void
    private providerType: string

    constructor(
        gateway: ModelGateway,
        options: {
            modelName?: string
            providerType?: string
            onEvent?: (event: ModelStreamEvent) => void
        } = {},
    ) {
        super({ callbacks: [], disableStreaming: true })
        this.gateway = gateway
        this.modelName = options.modelName ?? 'axolotl-agent:window'
        this.providerType = options.providerType ?? 'axolotl-agent'
        this.onEvent = options.onEvent
    }

    _llmType() {
        return this.providerType
    }

    get callKeys() {
        return [...super.callKeys, 'tools']
    }

    bindTools(tools: BindToolsInput[]) {
        return this.withConfig({ tools: tools.map((item) => ({ ...convertToOpenAITool(item) })) })
    }

    async _generate(
        messages: BaseMessage[],
        options: this['ParsedCallOptions'],
    ): Promise<ChatResult> {
        options.signal?.throwIfAborted()
        const result = await this.gateway.modelTurn(
            { messages: messages.map(serializeMessage), tools: options.tools ?? [] },
            this.onEvent,
            options.signal,
        )
        options.signal?.throwIfAborted()
        if (result.role !== 'assistant')
            throw new Error('Model returned an invalid assistant response')
        const message = deserializeMessage(result)
        return { generations: [{ text: result.content, message }] }
    }
}
