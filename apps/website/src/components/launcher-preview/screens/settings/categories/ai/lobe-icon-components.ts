import { type Component, defineAsyncComponent } from 'vue'

// The app's apps/app-frontend/src/data/lobeIconComponents.ts over the subset of
// @lobehub/icons-static-svg (MIT, LobeHub) that the provider catalog, the
// combine wordmarks and the model mappings can resolve to; the SVGs are copied
// next to this file. Each icon is its own lazy chunk, as in the app.

const icon_ace = () => import('./icons/ace.svg?component')
const icon_adobe = () => import('./icons/adobe.svg?component')
const icon_ai2 = () => import('./icons/ai2.svg?component')
const icon_ai21 = () => import('./icons/ai21.svg?component')
const icon_ai21_brand_color = () => import('./icons/ai21-brand-color.svg?component')
const icon_ai302_color = () => import('./icons/ai302-color.svg?component')
const icon_ai302_text = () => import('./icons/ai302-text.svg?component')
const icon_ai360 = () => import('./icons/ai360.svg?component')
const icon_ai360_color = () => import('./icons/ai360-color.svg?component')
const icon_ai360_text = () => import('./icons/ai360-text.svg?component')
const icon_aihubmix_color = () => import('./icons/aihubmix-color.svg?component')
const icon_aihubmix_text = () => import('./icons/aihubmix-text.svg?component')
const icon_aimass_color = () => import('./icons/aimass-color.svg?component')
const icon_aimass_text = () => import('./icons/aimass-text.svg?component')
const icon_aionlabs = () => import('./icons/aionlabs.svg?component')
const icon_akashchat_color = () => import('./icons/akashchat-color.svg?component')
const icon_akashchat_text = () => import('./icons/akashchat-text.svg?component')
const icon_alibabacloud_color = () => import('./icons/alibabacloud-color.svg?component')
const icon_alibabacloud_text_cn = () => import('./icons/alibabacloud-text-cn.svg?component')
const icon_antgroup_text = () => import('./icons/antgroup-text.svg?component')
const icon_anthropic = () => import('./icons/anthropic.svg?component')
const icon_anthropic_text = () => import('./icons/anthropic-text.svg?component')
const icon_arcee = () => import('./icons/arcee.svg?component')
const icon_assemblyai = () => import('./icons/assemblyai.svg?component')
const icon_aws_color = () => import('./icons/aws-color.svg?component')
const icon_aya = () => import('./icons/aya.svg?component')
const icon_azure_color = () => import('./icons/azure-color.svg?component')
const icon_azure_text = () => import('./icons/azure-text.svg?component')
const icon_azureai_color = () => import('./icons/azureai-color.svg?component')
const icon_azureai_text = () => import('./icons/azureai-text.svg?component')
const icon_baai = () => import('./icons/baai.svg?component')
const icon_baichuan = () => import('./icons/baichuan.svg?component')
const icon_baichuan_color = () => import('./icons/baichuan-color.svg?component')
const icon_baichuan_text = () => import('./icons/baichuan-text.svg?component')
const icon_baiducloud = () => import('./icons/baiducloud.svg?component')
const icon_baiducloud_color = () => import('./icons/baiducloud-color.svg?component')
const icon_baiducloud_text = () => import('./icons/baiducloud-text.svg?component')
const icon_bailian_color = () => import('./icons/bailian-color.svg?component')
const icon_bailian_text = () => import('./icons/bailian-text.svg?component')
const icon_bedrock_color = () => import('./icons/bedrock-color.svg?component')
const icon_bedrock_text = () => import('./icons/bedrock-text.svg?component')
const icon_bilibili = () => import('./icons/bilibili.svg?component')
const icon_bilibiliindex = () => import('./icons/bilibiliindex.svg?component')
const icon_burncloud = () => import('./icons/burncloud.svg?component')
const icon_bytedance = () => import('./icons/bytedance.svg?component')
const icon_cerebras_brand_color = () => import('./icons/cerebras-brand-color.svg?component')
const icon_chatglm = () => import('./icons/chatglm.svg?component')
const icon_claude = () => import('./icons/claude.svg?component')
const icon_claude_color = () => import('./icons/claude-color.svg?component')
const icon_claude_text = () => import('./icons/claude-text.svg?component')
const icon_cloudflare_color = () => import('./icons/cloudflare-color.svg?component')
const icon_cloudflare_text = () => import('./icons/cloudflare-text.svg?component')
const icon_codegeex = () => import('./icons/codegeex.svg?component')
const icon_cogview_color = () => import('./icons/cogview-color.svg?component')
const icon_cohere = () => import('./icons/cohere.svg?component')
const icon_cohere_color = () => import('./icons/cohere-color.svg?component')
const icon_cohere_text = () => import('./icons/cohere-text.svg?component')
const icon_cometapi_color = () => import('./icons/cometapi-color.svg?component')
const icon_cometapi_text = () => import('./icons/cometapi-text.svg?component')
const icon_dbrx = () => import('./icons/dbrx.svg?component')
const icon_deepcogito = () => import('./icons/deepcogito.svg?component')
const icon_deepmind = () => import('./icons/deepmind.svg?component')
const icon_deepseek = () => import('./icons/deepseek.svg?component')
const icon_deepseek_color = () => import('./icons/deepseek-color.svg?component')
const icon_deepseek_text = () => import('./icons/deepseek-text.svg?component')
const icon_dolphin = () => import('./icons/dolphin.svg?component')
const icon_doubao_color = () => import('./icons/doubao-color.svg?component')
const icon_essentialai = () => import('./icons/essentialai.svg?component')
const icon_fireworks = () => import('./icons/fireworks.svg?component')
const icon_fireworks_color = () => import('./icons/fireworks-color.svg?component')
const icon_fireworks_text = () => import('./icons/fireworks-text.svg?component')
const icon_fishaudio = () => import('./icons/fishaudio.svg?component')
const icon_flux = () => import('./icons/flux.svg?component')
const icon_gemini_color = () => import('./icons/gemini-color.svg?component')
const icon_gemini_text = () => import('./icons/gemini-text.svg?component')
const icon_gemma = () => import('./icons/gemma.svg?component')
const icon_giteeai = () => import('./icons/giteeai.svg?component')
const icon_giteeai_text = () => import('./icons/giteeai-text.svg?component')
const icon_github = () => import('./icons/github.svg?component')
const icon_github_text = () => import('./icons/github-text.svg?component')
const icon_githubcopilot = () => import('./icons/githubcopilot.svg?component')
const icon_githubcopilot_text = () => import('./icons/githubcopilot-text.svg?component')
const icon_glmv = () => import('./icons/glmv.svg?component')
const icon_google_brand_color = () => import('./icons/google-brand-color.svg?component')
const icon_google_color = () => import('./icons/google-color.svg?component')
const icon_grok = () => import('./icons/grok.svg?component')
const icon_grok_text = () => import('./icons/grok-text.svg?component')
const icon_groq_text = () => import('./icons/groq-text.svg?component')
const icon_huggingface_color = () => import('./icons/huggingface-color.svg?component')
const icon_huggingface_text = () => import('./icons/huggingface-text.svg?component')
const icon_hunyuan = () => import('./icons/hunyuan.svg?component')
const icon_hunyuan_color = () => import('./icons/hunyuan-color.svg?component')
const icon_hunyuan_text = () => import('./icons/hunyuan-text.svg?component')
const icon_ibm = () => import('./icons/ibm.svg?component')
const icon_ideogram = () => import('./icons/ideogram.svg?component')
const icon_inception = () => import('./icons/inception.svg?component')
const icon_infinigence_color = () => import('./icons/infinigence-color.svg?component')
const icon_infinigence_text_cn = () => import('./icons/infinigence-text-cn.svg?component')
const icon_inflection = () => import('./icons/inflection.svg?component')
const icon_internlm = () => import('./icons/internlm.svg?component')
const icon_internlm_color = () => import('./icons/internlm-color.svg?component')
const icon_internlm_text = () => import('./icons/internlm-text.svg?component')
const icon_jimeng_color = () => import('./icons/jimeng-color.svg?component')
const icon_jina = () => import('./icons/jina.svg?component')
const icon_jina_text = () => import('./icons/jina-text.svg?component')
const icon_kling_color = () => import('./icons/kling-color.svg?component')
const icon_kolors = () => import('./icons/kolors.svg?component')
const icon_kwaipilot_color = () => import('./icons/kwaipilot-color.svg?component')
const icon_liquid = () => import('./icons/liquid.svg?component')
const icon_llava = () => import('./icons/llava.svg?component')
const icon_lmstudio_text = () => import('./icons/lmstudio-text.svg?component')
const icon_longcat_color = () => import('./icons/longcat-color.svg?component')
const icon_longcat_text = () => import('./icons/longcat-text.svg?component')
const icon_menlo = () => import('./icons/menlo.svg?component')
const icon_meta = () => import('./icons/meta.svg?component')
const icon_microsoft = () => import('./icons/microsoft.svg?component')
const icon_minimax = () => import('./icons/minimax.svg?component')
const icon_minimax_color = () => import('./icons/minimax-color.svg?component')
const icon_minimax_text = () => import('./icons/minimax-text.svg?component')
const icon_mistral = () => import('./icons/mistral.svg?component')
const icon_mistral_color = () => import('./icons/mistral-color.svg?component')
const icon_mistral_text = () => import('./icons/mistral-text.svg?component')
const icon_modelscope_color = () => import('./icons/modelscope-color.svg?component')
const icon_modelscope_text = () => import('./icons/modelscope-text.svg?component')
const icon_moonshot = () => import('./icons/moonshot.svg?component')
const icon_moonshot_text = () => import('./icons/moonshot-text.svg?component')
const icon_morph = () => import('./icons/morph.svg?component')
const icon_nebius_text = () => import('./icons/nebius-text.svg?component')
const icon_newapi_color = () => import('./icons/newapi-color.svg?component')
const icon_newapi_text = () => import('./icons/newapi-text.svg?component')
const icon_nousresearch = () => import('./icons/nousresearch.svg?component')
const icon_nova = () => import('./icons/nova.svg?component')
const icon_novita_color = () => import('./icons/novita-color.svg?component')
const icon_novita_text = () => import('./icons/novita-text.svg?component')
const icon_nvidia = () => import('./icons/nvidia.svg?component')
const icon_nvidia_color = () => import('./icons/nvidia-color.svg?component')
const icon_nvidia_text = () => import('./icons/nvidia-text.svg?component')
const icon_ollama = () => import('./icons/ollama.svg?component')
const icon_ollama_text = () => import('./icons/ollama-text.svg?component')
const icon_openai = () => import('./icons/openai.svg?component')
const icon_openai_text = () => import('./icons/openai-text.svg?component')
const icon_openchat = () => import('./icons/openchat.svg?component')
const icon_opencode_text = () => import('./icons/opencode-text.svg?component')
const icon_openrouter = () => import('./icons/openrouter.svg?component')
const icon_openrouter_color = () => import('./icons/openrouter-color.svg?component')
const icon_openrouter_text = () => import('./icons/openrouter-text.svg?component')
const icon_palm_color = () => import('./icons/palm-color.svg?component')
const icon_perplexity = () => import('./icons/perplexity.svg?component')
const icon_perplexity_color = () => import('./icons/perplexity-color.svg?component')
const icon_perplexity_text = () => import('./icons/perplexity-text.svg?component')
const icon_phind = () => import('./icons/phind.svg?component')
const icon_ppio_color = () => import('./icons/ppio-color.svg?component')
const icon_ppio_text = () => import('./icons/ppio-text.svg?component')
const icon_qiniu = () => import('./icons/qiniu.svg?component')
const icon_qiniu_color = () => import('./icons/qiniu-color.svg?component')
const icon_qiniu_text = () => import('./icons/qiniu-text.svg?component')
const icon_qwen = () => import('./icons/qwen.svg?component')
const icon_qwen_color = () => import('./icons/qwen-color.svg?component')
const icon_qwen_text = () => import('./icons/qwen-text.svg?component')
const icon_relace = () => import('./icons/relace.svg?component')
const icon_rwkv = () => import('./icons/rwkv.svg?component')
const icon_sambanova_color = () => import('./icons/sambanova-color.svg?component')
const icon_sambanova_text = () => import('./icons/sambanova-text.svg?component')
const icon_search1api_color = () => import('./icons/search1api-color.svg?component')
const icon_search1api_text = () => import('./icons/search1api-text.svg?component')
const icon_sensenova = () => import('./icons/sensenova.svg?component')
const icon_sensenova_color = () => import('./icons/sensenova-color.svg?component')
const icon_sensenova_text = () => import('./icons/sensenova-text.svg?component')
const icon_siliconcloud_color = () => import('./icons/siliconcloud-color.svg?component')
const icon_siliconcloud_text = () => import('./icons/siliconcloud-text.svg?component')
const icon_skywork_color = () => import('./icons/skywork-color.svg?component')
const icon_spark = () => import('./icons/spark.svg?component')
const icon_spark_color = () => import('./icons/spark-color.svg?component')
const icon_spark_text = () => import('./icons/spark-text.svg?component')
const icon_stability = () => import('./icons/stability.svg?component')
const icon_stepfun = () => import('./icons/stepfun.svg?component')
const icon_stepfun_text = () => import('./icons/stepfun-text.svg?component')
const icon_straico_color = () => import('./icons/straico-color.svg?component')
const icon_straico_text = () => import('./icons/straico-text.svg?component')
const icon_streamlake_color = () => import('./icons/streamlake-color.svg?component')
const icon_streamlake_text = () => import('./icons/streamlake-text.svg?component')
const icon_suno = () => import('./icons/suno.svg?component')
const icon_tencentcloud_color = () => import('./icons/tencentcloud-color.svg?component')
const icon_tencentcloud_text = () => import('./icons/tencentcloud-text.svg?component')
const icon_tii = () => import('./icons/tii.svg?component')
const icon_together_color = () => import('./icons/together-color.svg?component')
const icon_together_text = () => import('./icons/together-text.svg?component')
const icon_udio = () => import('./icons/udio.svg?component')
const icon_upstage = () => import('./icons/upstage.svg?component')
const icon_upstage_color = () => import('./icons/upstage-color.svg?component')
const icon_upstage_text = () => import('./icons/upstage-text.svg?component')
const icon_v0 = () => import('./icons/v0.svg?component')
const icon_vercel = () => import('./icons/vercel.svg?component')
const icon_vercel_text = () => import('./icons/vercel-text.svg?component')
const icon_vertexai = () => import('./icons/vertexai.svg?component')
const icon_vertexai_color = () => import('./icons/vertexai-color.svg?component')
const icon_vertexai_text = () => import('./icons/vertexai-text.svg?component')
const icon_vllm_color = () => import('./icons/vllm-color.svg?component')
const icon_vllm_text = () => import('./icons/vllm-text.svg?component')
const icon_volcengine_color = () => import('./icons/volcengine-color.svg?component')
const icon_volcengine_text = () => import('./icons/volcengine-text.svg?component')
const icon_voyage = () => import('./icons/voyage.svg?component')
const icon_wenxin = () => import('./icons/wenxin.svg?component')
const icon_wenxin_color = () => import('./icons/wenxin-color.svg?component')
const icon_wenxin_text = () => import('./icons/wenxin-text.svg?component')
const icon_workersai_color = () => import('./icons/workersai-color.svg?component')
const icon_workersai_text = () => import('./icons/workersai-text.svg?component')
const icon_xai_text = () => import('./icons/xai-text.svg?component')
const icon_xiaomimimo = () => import('./icons/xiaomimimo.svg?component')
const icon_xiaomimimo_text = () => import('./icons/xiaomimimo-text.svg?component')
const icon_xinference_color = () => import('./icons/xinference-color.svg?component')
const icon_xinference_text = () => import('./icons/xinference-text.svg?component')
const icon_yi_color = () => import('./icons/yi-color.svg?component')
const icon_zai = () => import('./icons/zai.svg?component')
const icon_zenmux = () => import('./icons/zenmux.svg?component')
const icon_zenmux_text = () => import('./icons/zenmux-text.svg?component')
const icon_zeroone_text = () => import('./icons/zeroone-text.svg?component')
const icon_zhipu_color = () => import('./icons/zhipu-color.svg?component')
const icon_zhipu_text = () => import('./icons/zhipu-text.svg?component')

type IconModule = { default: Component }

const loaders: Record<string, () => Promise<IconModule>> = {
    ace: icon_ace,
    adobe: icon_adobe,
    ai2: icon_ai2,
    ai21: icon_ai21,
    'ai21-brand-color': icon_ai21_brand_color,
    'ai302-color': icon_ai302_color,
    'ai302-text': icon_ai302_text,
    ai360: icon_ai360,
    'ai360-color': icon_ai360_color,
    'ai360-text': icon_ai360_text,
    'aihubmix-color': icon_aihubmix_color,
    'aihubmix-text': icon_aihubmix_text,
    'aimass-color': icon_aimass_color,
    'aimass-text': icon_aimass_text,
    aionlabs: icon_aionlabs,
    'akashchat-color': icon_akashchat_color,
    'akashchat-text': icon_akashchat_text,
    'alibabacloud-color': icon_alibabacloud_color,
    'alibabacloud-text-cn': icon_alibabacloud_text_cn,
    'antgroup-text': icon_antgroup_text,
    anthropic: icon_anthropic,
    'anthropic-text': icon_anthropic_text,
    arcee: icon_arcee,
    assemblyai: icon_assemblyai,
    'aws-color': icon_aws_color,
    aya: icon_aya,
    'azure-color': icon_azure_color,
    'azure-text': icon_azure_text,
    'azureai-color': icon_azureai_color,
    'azureai-text': icon_azureai_text,
    baai: icon_baai,
    baichuan: icon_baichuan,
    'baichuan-color': icon_baichuan_color,
    'baichuan-text': icon_baichuan_text,
    baiducloud: icon_baiducloud,
    'baiducloud-color': icon_baiducloud_color,
    'baiducloud-text': icon_baiducloud_text,
    'bailian-color': icon_bailian_color,
    'bailian-text': icon_bailian_text,
    'bedrock-color': icon_bedrock_color,
    'bedrock-text': icon_bedrock_text,
    bilibili: icon_bilibili,
    bilibiliindex: icon_bilibiliindex,
    burncloud: icon_burncloud,
    bytedance: icon_bytedance,
    'cerebras-brand-color': icon_cerebras_brand_color,
    chatglm: icon_chatglm,
    claude: icon_claude,
    'claude-color': icon_claude_color,
    'claude-text': icon_claude_text,
    'cloudflare-color': icon_cloudflare_color,
    'cloudflare-text': icon_cloudflare_text,
    codegeex: icon_codegeex,
    'cogview-color': icon_cogview_color,
    cohere: icon_cohere,
    'cohere-color': icon_cohere_color,
    'cohere-text': icon_cohere_text,
    'cometapi-color': icon_cometapi_color,
    'cometapi-text': icon_cometapi_text,
    dbrx: icon_dbrx,
    deepcogito: icon_deepcogito,
    deepmind: icon_deepmind,
    deepseek: icon_deepseek,
    'deepseek-color': icon_deepseek_color,
    'deepseek-text': icon_deepseek_text,
    dolphin: icon_dolphin,
    'doubao-color': icon_doubao_color,
    essentialai: icon_essentialai,
    fireworks: icon_fireworks,
    'fireworks-color': icon_fireworks_color,
    'fireworks-text': icon_fireworks_text,
    fishaudio: icon_fishaudio,
    flux: icon_flux,
    'gemini-color': icon_gemini_color,
    'gemini-text': icon_gemini_text,
    gemma: icon_gemma,
    giteeai: icon_giteeai,
    'giteeai-text': icon_giteeai_text,
    github: icon_github,
    'github-text': icon_github_text,
    githubcopilot: icon_githubcopilot,
    'githubcopilot-text': icon_githubcopilot_text,
    glmv: icon_glmv,
    'google-brand-color': icon_google_brand_color,
    'google-color': icon_google_color,
    grok: icon_grok,
    'grok-text': icon_grok_text,
    'groq-text': icon_groq_text,
    'huggingface-color': icon_huggingface_color,
    'huggingface-text': icon_huggingface_text,
    hunyuan: icon_hunyuan,
    'hunyuan-color': icon_hunyuan_color,
    'hunyuan-text': icon_hunyuan_text,
    ibm: icon_ibm,
    ideogram: icon_ideogram,
    inception: icon_inception,
    'infinigence-color': icon_infinigence_color,
    'infinigence-text-cn': icon_infinigence_text_cn,
    inflection: icon_inflection,
    internlm: icon_internlm,
    'internlm-color': icon_internlm_color,
    'internlm-text': icon_internlm_text,
    'jimeng-color': icon_jimeng_color,
    jina: icon_jina,
    'jina-text': icon_jina_text,
    'kling-color': icon_kling_color,
    kolors: icon_kolors,
    'kwaipilot-color': icon_kwaipilot_color,
    liquid: icon_liquid,
    llava: icon_llava,
    'lmstudio-text': icon_lmstudio_text,
    'longcat-color': icon_longcat_color,
    'longcat-text': icon_longcat_text,
    menlo: icon_menlo,
    meta: icon_meta,
    microsoft: icon_microsoft,
    minimax: icon_minimax,
    'minimax-color': icon_minimax_color,
    'minimax-text': icon_minimax_text,
    mistral: icon_mistral,
    'mistral-color': icon_mistral_color,
    'mistral-text': icon_mistral_text,
    'modelscope-color': icon_modelscope_color,
    'modelscope-text': icon_modelscope_text,
    moonshot: icon_moonshot,
    'moonshot-text': icon_moonshot_text,
    morph: icon_morph,
    'nebius-text': icon_nebius_text,
    'newapi-color': icon_newapi_color,
    'newapi-text': icon_newapi_text,
    nousresearch: icon_nousresearch,
    nova: icon_nova,
    'novita-color': icon_novita_color,
    'novita-text': icon_novita_text,
    nvidia: icon_nvidia,
    'nvidia-color': icon_nvidia_color,
    'nvidia-text': icon_nvidia_text,
    ollama: icon_ollama,
    'ollama-text': icon_ollama_text,
    openai: icon_openai,
    'openai-text': icon_openai_text,
    openchat: icon_openchat,
    'opencode-text': icon_opencode_text,
    openrouter: icon_openrouter,
    'openrouter-color': icon_openrouter_color,
    'openrouter-text': icon_openrouter_text,
    'palm-color': icon_palm_color,
    perplexity: icon_perplexity,
    'perplexity-color': icon_perplexity_color,
    'perplexity-text': icon_perplexity_text,
    phind: icon_phind,
    'ppio-color': icon_ppio_color,
    'ppio-text': icon_ppio_text,
    qiniu: icon_qiniu,
    'qiniu-color': icon_qiniu_color,
    'qiniu-text': icon_qiniu_text,
    qwen: icon_qwen,
    'qwen-color': icon_qwen_color,
    'qwen-text': icon_qwen_text,
    relace: icon_relace,
    rwkv: icon_rwkv,
    'sambanova-color': icon_sambanova_color,
    'sambanova-text': icon_sambanova_text,
    'search1api-color': icon_search1api_color,
    'search1api-text': icon_search1api_text,
    sensenova: icon_sensenova,
    'sensenova-color': icon_sensenova_color,
    'sensenova-text': icon_sensenova_text,
    'siliconcloud-color': icon_siliconcloud_color,
    'siliconcloud-text': icon_siliconcloud_text,
    'skywork-color': icon_skywork_color,
    spark: icon_spark,
    'spark-color': icon_spark_color,
    'spark-text': icon_spark_text,
    stability: icon_stability,
    stepfun: icon_stepfun,
    'stepfun-text': icon_stepfun_text,
    'straico-color': icon_straico_color,
    'straico-text': icon_straico_text,
    'streamlake-color': icon_streamlake_color,
    'streamlake-text': icon_streamlake_text,
    suno: icon_suno,
    'tencentcloud-color': icon_tencentcloud_color,
    'tencentcloud-text': icon_tencentcloud_text,
    tii: icon_tii,
    'together-color': icon_together_color,
    'together-text': icon_together_text,
    udio: icon_udio,
    upstage: icon_upstage,
    'upstage-color': icon_upstage_color,
    'upstage-text': icon_upstage_text,
    v0: icon_v0,
    vercel: icon_vercel,
    'vercel-text': icon_vercel_text,
    vertexai: icon_vertexai,
    'vertexai-color': icon_vertexai_color,
    'vertexai-text': icon_vertexai_text,
    'vllm-color': icon_vllm_color,
    'vllm-text': icon_vllm_text,
    'volcengine-color': icon_volcengine_color,
    'volcengine-text': icon_volcengine_text,
    voyage: icon_voyage,
    wenxin: icon_wenxin,
    'wenxin-color': icon_wenxin_color,
    'wenxin-text': icon_wenxin_text,
    'workersai-color': icon_workersai_color,
    'workersai-text': icon_workersai_text,
    'xai-text': icon_xai_text,
    xiaomimimo: icon_xiaomimimo,
    'xiaomimimo-text': icon_xiaomimimo_text,
    'xinference-color': icon_xinference_color,
    'xinference-text': icon_xinference_text,
    'yi-color': icon_yi_color,
    zai: icon_zai,
    zenmux: icon_zenmux,
    'zenmux-text': icon_zenmux_text,
    'zeroone-text': icon_zeroone_text,
    'zhipu-color': icon_zhipu_color,
    'zhipu-text': icon_zhipu_text,
}

/** The async component of one icon; `cache` is the caller's, so nothing is module state. */
export function getLobeIconComponent(
    slug: string,
    cache: Map<string, Component>,
): Component | undefined {
    const cached = cache.get(slug)
    if (cached) return cached

    const loader = loaders[slug]
    if (!loader) return undefined

    const component = defineAsyncComponent(async () => {
        const loaded = await loader()
        return loaded.default
    })
    cache.set(slug, component)
    return component
}

export function hasLobeIcon(slug: string): boolean {
    return slug in loaders
}
