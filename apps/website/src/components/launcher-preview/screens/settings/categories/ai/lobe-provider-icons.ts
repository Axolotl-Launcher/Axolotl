// Copied from apps/app-frontend/src/data/lobeProviderIcons.ts.

export type LobeAvatarAsset = 'color' | 'mono'

export interface LobeAvatarConfig {
    asset: LobeAvatarAsset
    background: string
    color: string
    multiple: number
    variant?: 'ai302' | 'aihubmix' | 'stepfun'
}

export type LobeCombineKind =
    | 'anthropic'
    | 'azure'
    | 'bedrock'
    | 'cloudflare'
    | 'generic'
    | 'google'
    | 'ollamacloud'
    | 'qwen'
    | 'standalone'
    | 'v0'
    | 'wenxin'

export interface LobeCombineConfig {
    brand?: string
    color?: string
    kind: LobeCombineKind
    multiple: number
}

export interface LobeProviderIconConfig {
    avatar: LobeAvatarConfig
    combine: LobeCombineConfig
    slug: string
}

export interface LobeCombineBrandConfig {
    avatar?: boolean
    color?: string
    inverse?: boolean
    logo?: string
    spaceMultiple?: number
    standalone?: string
    text?: string
    textMultiple?: number
}

const avatar = (
    background: string,
    color: string,
    multiple: number,
    asset: LobeAvatarAsset = 'mono',
    variant?: LobeAvatarConfig['variant'],
): LobeAvatarConfig => ({ asset, background, color, multiple, variant })

export const lobeAvatarBrands: Record<string, LobeAvatarConfig> = {
    ai21: avatar('#E91E63', '#fff', 0.7),
    ai302: avatar('#8E47FF', '#fff', 0.8, 'mono', 'ai302'),
    ai360: avatar('linear-gradient(to bottom, #12B7FA, #006FFB)', '#fff', 0.75),
    aihubmix: avatar('#006FFB', '#fff', 0.75, 'mono', 'aihubmix'),
    aimass: avatar('#fff', '#fff', 0.8, 'color'),
    akashchat: avatar('#000', '#fff', 0.75, 'color'),
    antgroup: avatar('#1677FF', '#fff', 0.8),
    anthropic: avatar('#F1F0E8', '#141413', 0.75),
    azure: avatar('#fff', '#fff', 0.7, 'color'),
    azureai: avatar('#000', '#fff', 0.6, 'color'),
    baichuan: avatar('#FF6933', '#fff', 0.6),
    bailian: avatar('#fff', '#fff', 0.75, 'color'),
    bedrock: avatar('linear-gradient(45deg, #9AD8F8, #3D8FFF, #6350FB)', '#fff', 0.75),
    cerebras: avatar('#F15A29', '#fff', 0.8),
    cloudflare: avatar('#F38020', '#fff', 0.75),
    cohere: avatar('#39594D', '#fff', 0.6),
    cometapi: avatar('#fff', '#00ACE2', 0.8, 'color'),
    deepseek: avatar('#4D6BFE', '#fff', 0.75),
    fireworks: avatar('#5019C5', '#000', 0.75),
    giteeai: avatar('#000', '#fff', 0.75),
    github: avatar('#000', '#fff', 0.75),
    githubcopilot: avatar('#000', '#fff', 0.75),
    google: avatar('#fff', '#fff', 0.75, 'color'),
    grok: avatar('#000', '#fff', 0.75),
    groq: avatar('#F55036', '#fff', 0.75),
    higress: avatar('linear-gradient(to bottom, #0418FF, #1E8CFE)', '#fff', 0.6),
    huggingface: avatar('#fff', '#fff', 0.75, 'color'),
    hunyuan: avatar('#0053E0', '#fff', 0.75),
    infinigence: avatar('#7952EA', '#fff', 0.6),
    internlm: avatar('#1B3882', '#fff', 0.75),
    jina: avatar('#000', '#fff', 0.6),
    lmstudio: avatar('linear-gradient(135deg, #6C78EF, #4F14BE)', '#fff', 0.7),
    longcat: avatar('#fff', '#000', 0.7, 'color'),
    minimax: avatar('linear-gradient(to right, #E2167E, #FE603C)', '#fff', 0.75),
    mistral: avatar('#FA520F', '#fff', 0.75),
    modelscope: avatar('#624AFF', '#fff', 0.75),
    moonshot: avatar('#16191E', '#fff', 0.75),
    nebius: avatar('#DAFF33', '#052B42', 0.6),
    newapi: avatar('#fff', '#DD2E57', 0.7, 'color'),
    novita: avatar('#23D57C', '#000', 0.75),
    nvidia: avatar('#74B71B', '#fff', 0.75),
    ollama: avatar('#fff', '#000', 0.75),
    openai: avatar('#000', '#fff', 0.75),
    opencode: avatar('#000', '#fff', 0.75),
    openrouter: avatar('#000', '#C8FF00', 0.75),
    perplexity: avatar('#22B8CD', '#000', 0.75),
    ppio: avatar('#2874FF', '#fff', 0.75),
    qiniu: avatar('#06AEEF', '#fff', 0.75),
    alibabacloud: avatar('#FF6A00', '#fff', 0.7),
    sambanova: avatar('#EE7624', '#fff', 0.6),
    search1api: avatar('#0066FF', '#fff', 0.6),
    sensenova: avatar('#5B2AD8', '#fff', 0.7),
    siliconcloud: avatar('#6E29F6', '#fff', 0.7),
    spark: avatar('#0070F0', '#fff', 0.75),
    stepfun: avatar('#fff', '#000', 0.65, 'mono', 'stepfun'),
    straico: avatar('#464BBA', '#fff', 0.6),
    streamlake: avatar('#1D70FF', '#fff', 0.7),
    tencentcloud: avatar('#2151D1', '#fff', 0.75),
    together: avatar('#fff', '#000', 0.75, 'color'),
    upstage: avatar('linear-gradient(to bottom, #AEBCFE, #805DFA)', '#fff', 0.6),
    vercel: avatar('#000', '#fff', 0.6),
    vertexai: avatar('#4285F4', '#fff', 0.6),
    vllm: avatar('#fff', '#fff', 0.6, 'color'),
    volcengine: avatar('#fff', '#fff', 0.75, 'color'),
    wenxin: avatar('linear-gradient(to right, #0A51C3, #23A4FB)', '#fff', 0.75),
    xai: avatar('#fff', '#000', 0.65),
    xiaomimimo: avatar('#000', '#fff', 0.7),
    xinference: avatar('#781FF5', '#fff', 0.7),
    zenmux: avatar('#000', '#fff', 0.7),
    zeroone: avatar('#003425', '#fff', 0.6, 'color'),
    zhipu: avatar('#3859FF', '#fff', 0.75),
}

export const lobeCombineBrands: Record<string, LobeCombineBrandConfig> = {
    ai21: { standalone: 'ai21-brand-color', textMultiple: 0.75 },
    ai302: { logo: 'ai302-color', spaceMultiple: 0.15, text: 'ai302-text', textMultiple: 0.8 },
    ai360: { logo: 'ai360-color', spaceMultiple: 0.2, text: 'ai360-text', textMultiple: 0.7 },
    aihubmix: {
        logo: 'aihubmix-color',
        spaceMultiple: 0.2,
        text: 'aihubmix-text',
        textMultiple: 0.7,
    },
    aimass: { logo: 'aimass-color', spaceMultiple: 0.2, text: 'aimass-text', textMultiple: 0.65 },
    akashchat: {
        logo: 'akashchat-color',
        spaceMultiple: 0.1,
        text: 'akashchat-text',
        textMultiple: 0.9,
    },
    antgroup: { standalone: 'antgroup-text' },
    azure: { logo: 'azure-color', spaceMultiple: 0.25, text: 'azure-text', textMultiple: 0.75 },
    azureai: {
        logo: 'azureai-color',
        spaceMultiple: 0.2,
        text: 'azureai-text',
        textMultiple: 0.7,
    },
    baichuan: {
        logo: 'baichuan-color',
        spaceMultiple: 0.2,
        text: 'baichuan-text',
        textMultiple: 1,
    },
    bailian: { logo: 'bailian-color', spaceMultiple: 0.2, text: 'bailian-text', textMultiple: 0.8 },
    baiducloud: {
        logo: 'baiducloud-color',
        spaceMultiple: 0.15,
        text: 'baiducloud-text',
        textMultiple: 0.8,
    },
    bedrock: {
        logo: 'bedrock-color',
        spaceMultiple: 0.1,
        text: 'bedrock-text',
        textMultiple: 0.6,
    },
    cerebras: { standalone: 'cerebras-brand-color' },
    claude: { logo: 'claude-color', spaceMultiple: 0.1, text: 'claude-text', textMultiple: 0.8 },
    cloudflare: {
        logo: 'cloudflare-color',
        spaceMultiple: 0.25,
        text: 'cloudflare-text',
        textMultiple: 0.4,
    },
    cohere: { logo: 'cohere-color', spaceMultiple: 0.3, text: 'cohere-text', textMultiple: 0.75 },
    cometapi: {
        logo: 'cometapi-color',
        spaceMultiple: 0.2,
        text: 'cometapi-text',
        textMultiple: 0.75,
    },
    deepseek: {
        logo: 'deepseek-color',
        spaceMultiple: 0.2,
        text: 'deepseek-text',
        textMultiple: 0.65,
    },
    fireworks: {
        logo: 'fireworks-color',
        spaceMultiple: 0.2,
        text: 'fireworks-text',
        textMultiple: 0.6,
    },
    gemini: { logo: 'gemini-color', spaceMultiple: 0.2, text: 'gemini-text', textMultiple: 0.8 },
    giteeai: { logo: 'giteeai', spaceMultiple: 0.2, text: 'giteeai-text', textMultiple: 0.85 },
    github: { logo: 'github', spaceMultiple: 0.2, text: 'github-text', textMultiple: 0.8 },
    githubcopilot: {
        logo: 'githubcopilot',
        spaceMultiple: 0.3,
        text: 'githubcopilot-text',
        textMultiple: 0.75,
    },
    grok: { logo: 'grok', spaceMultiple: 0.2, text: 'grok-text', textMultiple: 0.75 },
    groq: { standalone: 'groq-text', textMultiple: 0.75 },
    higress: { standalone: 'higress-combine' },
    huggingface: {
        logo: 'huggingface-color',
        spaceMultiple: 0.3,
        text: 'huggingface-text',
        textMultiple: 0.6,
    },
    hunyuan: {
        logo: 'hunyuan-color',
        spaceMultiple: 0.2,
        text: 'hunyuan-text',
        textMultiple: 0.75,
    },
    infinigence: {
        logo: 'infinigence-color',
        spaceMultiple: 0.2,
        text: 'infinigence-text-cn',
        textMultiple: 0.8,
    },
    internlm: {
        logo: 'internlm-color',
        spaceMultiple: 0.15,
        text: 'internlm-text',
        textMultiple: 0.75,
    },
    jina: { standalone: 'jina-text' },
    lmstudio: { avatar: true, spaceMultiple: 0.3, text: 'lmstudio-text', textMultiple: 0.6 },
    longcat: { logo: 'longcat-color', spaceMultiple: 0.3, text: 'longcat-text', textMultiple: 0.8 },
    minimax: {
        logo: 'minimax-color',
        spaceMultiple: 0.15,
        text: 'minimax-text',
        textMultiple: 0.45,
    },
    mistral: { logo: 'mistral-color', spaceMultiple: 0.2, text: 'mistral-text', textMultiple: 0.6 },
    modelscope: {
        logo: 'modelscope-color',
        spaceMultiple: 0.2,
        text: 'modelscope-text',
        textMultiple: 0.6,
    },
    moonshot: { logo: 'moonshot', spaceMultiple: 0.4, text: 'moonshot-text', textMultiple: 0.75 },
    nebius: { standalone: 'nebius-text' },
    newapi: { logo: 'newapi-color', spaceMultiple: 0.3, text: 'newapi-text', textMultiple: 0.8 },
    novita: { logo: 'novita-color', spaceMultiple: 0.25, text: 'novita-text', textMultiple: 0.7 },
    nvidia: { logo: 'nvidia-color', spaceMultiple: 0.15, text: 'nvidia-text', textMultiple: 0.5 },
    ollama: { logo: 'ollama', spaceMultiple: 0.1, text: 'ollama-text', textMultiple: 0.6 },
    openai: { logo: 'openai', spaceMultiple: 0.1, text: 'openai-text', textMultiple: 0.75 },
    opencode: { standalone: 'opencode-text', textMultiple: 0.6 },
    openrouter: {
        logo: 'openrouter-color',
        spaceMultiple: 0.3,
        text: 'openrouter-text',
        textMultiple: 1,
    },
    perplexity: {
        logo: 'perplexity-color',
        spaceMultiple: 0.2,
        text: 'perplexity-text',
        textMultiple: 0.75,
    },
    ppio: { logo: 'ppio-color', spaceMultiple: 0.3, text: 'ppio-text', textMultiple: 0.9 },
    qiniu: { logo: 'qiniu-color', spaceMultiple: 0.1, text: 'qiniu-text', textMultiple: 0.7 },
    qwen: { logo: 'qwen-color', spaceMultiple: 0.2, text: 'qwen-text', textMultiple: 0.7 },
    alibabacloud: {
        logo: 'alibabacloud-color',
        spaceMultiple: 0.2,
        text: 'alibabacloud-text-cn',
        textMultiple: 0.65,
    },
    sambanova: {
        logo: 'sambanova-color',
        spaceMultiple: 0.2,
        text: 'sambanova-text',
        textMultiple: 0.8,
    },
    search1api: {
        logo: 'search1api-color',
        spaceMultiple: 0.3,
        text: 'search1api-text',
        textMultiple: 0.65,
    },
    sensenova: {
        logo: 'sensenova-color',
        spaceMultiple: 0.2,
        text: 'sensenova-text',
        textMultiple: 0.8,
    },
    siliconcloud: {
        logo: 'siliconcloud-color',
        spaceMultiple: 0.2,
        text: 'siliconcloud-text',
        textMultiple: 0.7,
    },
    spark: { logo: 'spark-color', spaceMultiple: 0.2, text: 'spark-text', textMultiple: 0.75 },
    stepfun: { logo: 'stepfun', spaceMultiple: 0.3, text: 'stepfun-text', textMultiple: 0.9 },
    straico: { logo: 'straico-color', spaceMultiple: 0.1, text: 'straico-text', textMultiple: 0.9 },
    streamlake: {
        logo: 'streamlake-color',
        spaceMultiple: 0.2,
        text: 'streamlake-text',
        textMultiple: 0.75,
    },
    tencentcloud: {
        logo: 'tencentcloud-color',
        spaceMultiple: 0.2,
        text: 'tencentcloud-text',
        textMultiple: 0.75,
    },
    together: {
        logo: 'together-color',
        spaceMultiple: 0.2,
        text: 'together-text',
        textMultiple: 0.85,
    },
    upstage: { logo: 'upstage-color', spaceMultiple: 0.15, text: 'upstage-text', textMultiple: 1 },
    vercel: { logo: 'vercel', spaceMultiple: 0.05, text: 'vercel-text', textMultiple: 0.8 },
    vertexai: {
        logo: 'vertexai-color',
        spaceMultiple: 0.2,
        text: 'vertexai-text',
        textMultiple: 0.6,
    },
    vllm: { logo: 'vllm-color', spaceMultiple: 0.3, text: 'vllm-text', textMultiple: 0.85 },
    volcengine: {
        logo: 'volcengine-color',
        spaceMultiple: 0.2,
        text: 'volcengine-text',
        textMultiple: 0.8,
    },
    wenxin: { logo: 'wenxin-color', spaceMultiple: 0.2, text: 'wenxin-text', textMultiple: 0.75 },
    workersai: {
        logo: 'workersai-color',
        spaceMultiple: 0.2,
        text: 'workersai-text',
        textMultiple: 0.6,
    },
    xai: { avatar: true, spaceMultiple: 0.25, text: 'xai-text', textMultiple: 0.75 },
    xiaomimimo: { standalone: 'xiaomimimo-text' },
    xinference: {
        logo: 'xinference-color',
        spaceMultiple: 0.3,
        text: 'xinference-text',
        textMultiple: 0.7,
    },
    zenmux: {
        inverse: true,
        logo: 'zenmux',
        spaceMultiple: 0.1,
        text: 'zenmux-text',
        textMultiple: 1,
    },
    zeroone: { color: '#003425', standalone: 'zeroone-text', textMultiple: 0.8 },
    zhipu: { logo: 'zhipu-color', spaceMultiple: 0.2, text: 'zhipu-text', textMultiple: 0.65 },
}

const provider = (
    slug: string,
    multiple = 1,
    kind: LobeCombineKind = 'generic',
    brand = slug,
): LobeProviderIconConfig => ({
    avatar: lobeAvatarBrands[slug],
    combine: { brand, kind, multiple },
    slug,
})

export const lobeProviderIcons: Record<string, LobeProviderIconConfig> = {
    ai21: provider('ai21', 0.9),
    ai302: provider('ai302', 0.9),
    ai360: provider('ai360', 0.83),
    aihubmix: provider('aihubmix', 0.9),
    akashchat: provider('akashchat', 0.8),
    antgroup: provider('antgroup', 1, 'standalone'),
    anthropic: provider('anthropic', 0.83, 'anthropic'),
    azure: provider('azure', 0.9, 'azure'),
    azureai: provider('azureai'),
    baichuan: provider('baichuan', 0.83),
    bailiancodingplan: provider('bailian'),
    bedrock: provider('bedrock', 1.1, 'bedrock'),
    cerebras: provider('cerebras'),
    chatgpt: provider('openai'),
    cloudflare: provider('cloudflare', 1.1, 'cloudflare'),
    cohere: provider('cohere'),
    cometapi: provider('cometapi'),
    deepseek: provider('deepseek', 1.16),
    fireworksai: provider('fireworks', 1.14),
    giteeai: provider('giteeai', 0.95),
    github: provider('github', 0.95),
    githubcopilot: provider('githubcopilot', 0.95),
    glmcodingplan: provider('zhipu', 1.25),
    google: provider('google', 0.92, 'google'),
    groq: provider('groq'),
    higress: provider('higress'),
    huggingface: provider('huggingface', 1.16),
    hunyuan: provider('hunyuan'),
    infiniai: provider('infinigence', 0.8),
    internlm: provider('internlm', 0.95),
    jina: provider('jina', 1, 'standalone'),
    kimicodingplan: provider('moonshot', 0.9),
    lmstudio: provider('lmstudio'),
    longcat: provider('longcat'),
    minimax: provider('minimax', 1.3),
    minimaxcodingplan: provider('minimax', 1.3),
    mistral: provider('mistral'),
    modelscope: provider('modelscope', 1.2),
    moonshot: provider('moonshot', 0.9),
    nebius: provider('nebius', 0.75, 'standalone'),
    newapi: provider('newapi', 0.85),
    novita: provider('novita'),
    nvidia: provider('nvidia'),
    ollama: provider('ollama', 1.16),
    ollamacloud: provider('ollama', 1, 'ollamacloud'),
    openai: provider('openai'),
    opencodecodingplan: provider('opencode'),
    opencodezen: provider('opencode'),
    openrouter: provider('openrouter', 0.8),
    perplexity: provider('perplexity'),
    ppio: provider('ppio', 0.85),
    qiniu: provider('qiniu', 1.1),
    qwen: provider('alibabacloud', 1.1, 'qwen', 'qwen'),
    sambanova: provider('sambanova', 0.8),
    search1api: provider('search1api', 0.9),
    sensenova: provider('sensenova', 0.95),
    siliconcloud: provider('siliconcloud'),
    spark: provider('spark', 0.92),
    stepfun: provider('stepfun', 0.83),
    straico: provider('straico', 0.85),
    streamlake: provider('streamlake'),
    supergrok: provider('grok'),
    taichu: provider('aimass', 1.16),
    tencentcloud: provider('tencentcloud'),
    togetherai: provider('together'),
    upstage: provider('upstage', 0.9),
    v0: provider('vercel', 1, 'v0'),
    vercelaigateway: provider('vercel', 0.85),
    vertexai: provider('vertexai'),
    vllm: provider('vllm', 0.85),
    volcengine: provider('volcengine'),
    volcenginecodingplan: provider('volcengine'),
    wenxin: provider('wenxin', 1, 'wenxin'),
    xai: provider('xai', 0.85),
    xiaomimimo: provider('xiaomimimo', 0.7, 'standalone'),
    xinference: provider('xinference', 0.85),
    zenmux: provider('zenmux'),
    zeroone: provider('zeroone'),
    zhipu: provider('zhipu', 1.25),
}
