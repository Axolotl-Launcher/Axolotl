export const FALLEN_AUTH_PROXY_BLOG_URL =
    'https://blog.fallenbreath.me/zh-CN/2025/minecraft-service-proxy'

export const FALLEN_AUTH_PROXY_JAVA_ARGS = [
    '-Dminecraft.api.auth.host=https://auth.msp.fallenbreath.me',
    '-Dminecraft.api.account.host=https://account.msp.fallenbreath.me',
    '-Dminecraft.api.session.host=https://session.msp.fallenbreath.me',
    '-Dminecraft.api.services.host=https://services.msp.fallenbreath.me',
    '-Dminecraft.api.profiles.host=https://profiles.msp.fallenbreath.me',
]

export const FALLEN_AUTH_PROXY_JAVA_ARGS_STRING = FALLEN_AUTH_PROXY_JAVA_ARGS.join(' ')

export const AUTO_GC_PRESET_ARG = '@axolotl:gc:auto'
