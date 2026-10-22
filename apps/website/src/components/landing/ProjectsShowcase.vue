<script setup lang="ts">
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

const { formatMessage } = useVIntl()

interface ShowcaseProject {
    id: string
    iconUrl: string
    title: string
    description: string
}

// 真实 Modrinth 项目（id 与 CDN 图标来自公开 API）
const projects: ShowcaseProject[] = [
    {
        id: 'AANobbMI',
        title: 'Sodium',
        iconUrl:
            'https://cdn.modrinth.com/data/AANobbMI/295862f4724dc3f78df3447ad6072b2dcd3ef0c9_96.webp',
    },
    {
        id: 'YL57xq9U',
        title: 'Iris Shaders',
        iconUrl:
            'https://cdn.modrinth.com/data/YL57xq9U/18d0e7f076d3d6ed5bedd472b853909aac5da202_96.webp',
    },
    {
        id: 'gvQqBUqZ',
        title: 'Lithium',
        iconUrl:
            'https://cdn.modrinth.com/data/gvQqBUqZ/bcc8686c13af0143adf4285d741256af824f70b7_96.webp',
    },
    {
        id: 'mOgUt4GM',
        title: 'Mod Menu',
        iconUrl:
            'https://cdn.modrinth.com/data/mOgUt4GM/5a20ed1450a0e1e79a1fe04e61bb4e5878bf1d20.png',
    },
    {
        id: 'P7dR8mSH',
        title: 'Fabric API',
        iconUrl: 'https://cdn.modrinth.com/data/P7dR8mSH/icon.png',
    },
    {
        id: '9s6osm5g',
        title: 'Cloth Config API',
        iconUrl:
            'https://cdn.modrinth.com/data/9s6osm5g/ed8a2316cbb6f4fc5f510e8e13a59a85cbbbff4d_96.webp',
    },
    {
        id: 'lhGA9TYQ',
        title: 'Architectury API',
        iconUrl:
            'https://cdn.modrinth.com/data/lhGA9TYQ/05fe3a61c28faaccaec3533b92e1b321edde7bf6_96.webp',
    },
    {
        id: 'nrJ2NpD0',
        title: 'Craftify',
        iconUrl:
            'https://cdn.modrinth.com/data/nrJ2NpD0/4f21214db060ed4542b1f3983c4113d293480a1b.webp',
    },
    {
        id: 'u6dRKJwZ',
        title: 'Just Enough Items (JEI)',
        iconUrl:
            'https://cdn.modrinth.com/data/u6dRKJwZ/4a3f18ac0d096c9f8e9176984c44be4e58f94c89_96.webp',
    },
    {
        id: 'LNytGWDc',
        title: 'Create',
        iconUrl:
            'https://cdn.modrinth.com/data/LNytGWDc/61d716699bcf1ec42ed4926a9e1c7311be6087e2_96.webp',
    },
    {
        id: '1bokaNcj',
        title: "Xaero's Minimap",
        iconUrl:
            'https://cdn.modrinth.com/data/1bokaNcj/354080f65407e49f486fcf9c4580e82c45ae63b8_96.webp',
    },
    {
        id: '8oi3bsk5',
        title: 'Terralith',
        iconUrl:
            'https://cdn.modrinth.com/data/8oi3bsk5/1959d924a1088944bbf07a06ba523726112d7e7a_96.webp',
    },
    {
        id: 'NNAgCjsB',
        title: 'Entity Culling',
        iconUrl:
            'https://cdn.modrinth.com/data/NNAgCjsB/7873452d6cede4daed12da3d7d8c193ab88b4fd6_96.webp',
    },
    {
        id: 'uXXizFIs',
        title: 'FerriteCore',
        iconUrl:
            'https://cdn.modrinth.com/data/uXXizFIs/222a126f26f8f9ae1eb339f3b767677f18bff31f_96.webp',
    },
    {
        id: 'fRiHVvU7',
        title: 'EMI',
        iconUrl:
            'https://cdn.modrinth.com/data/fRiHVvU7/395fe5302b2bab612ef0623509f768f3c5a5ee0f.webp',
    },
    {
        id: '9eGKb6K1',
        title: 'Simple Voice Chat',
        iconUrl: 'https://cdn.modrinth.com/data/9eGKb6K1/icon.png',
    },
    {
        id: 'gu7yAYhd',
        title: 'CC: Tweaked',
        iconUrl: 'https://cdn.modrinth.com/data/gu7yAYhd/icon.png',
    },
    {
        id: 'EsAfCjCV',
        title: 'AppleSkin',
        iconUrl: 'https://cdn.modrinth.com/data/EsAfCjCV/icon.png',
    },
    {
        id: 'w7ThoJFB',
        title: 'Zoomify (Zoom)',
        iconUrl:
            'https://cdn.modrinth.com/data/w7ThoJFB/e2de67a0bfb9e8aa2347982ab3ec5463f26cca31_96.webp',
    },
    {
        id: 'nvQzSEkH',
        title: 'Jade',
        iconUrl:
            'https://cdn.modrinth.com/data/nvQzSEkH/b04217bc2b7dc524c4d12f81ff42cc1cefb9b0fc_96.webp',
    },
].map((project) => ({
    ...project,
    description: `${project.title} is available through Axolotl's content browser.`,
}))

// 每行渲染两份内容保证动画无缝衔接；后半轮是视觉重复，对屏幕阅读器隐藏
const showcaseProjects = [
    ...projects.map((project) => ({ ...project, isVisualDuplicate: false })),
    ...projects.map((project) => ({
        ...project,
        id: `${project.id}-repeat`,
        isVisualDuplicate: true,
    })),
]

const rowCount = 5
const perRow = Math.ceil(showcaseProjects.length / rowCount)
const rows = Array.from({ length: rowCount }, (_, index) =>
    showcaseProjects.slice(index * perRow, (index + 1) * perRow),
)

const messages = defineMessages({
    title: {
        id: 'app-marketing.features.website.title',
        defaultMessage: 'From discovery to install',
    },
    description: {
        id: 'app-marketing.features.website.description',
        defaultMessage:
            'Find something you want to play, choose a version, and install it directly into your game.',
    },
})
</script>

<template>
    <div class="projects-showcase">
        <img class="website-logo" src="/axolotl.png" alt="" aria-hidden="true" />
        <div v-for="(row, index) in rows" :key="index" class="row">
            <div v-for="n in 2" :key="n" class="row__content" :class="{ offset: index % 2 }">
                <div
                    v-for="project in row"
                    :key="project.id"
                    class="project"
                    :aria-hidden="project.isVisualDuplicate ? 'true' : undefined"
                >
                    <Avatar :src="project.iconUrl" alt="" size="sm" />
                    <div class="project-info">
                        <span class="title">{{ project.title }}</span>
                        <span class="description">{{ project.description }}</span>
                    </div>
                </div>
            </div>
        </div>
        <h3>{{ formatMessage(messages.title) }}</h3>
        <p>{{ formatMessage(messages.description) }}</p>
    </div>
</template>

<style lang="scss" scoped>
.projects-showcase {
    margin: calc(5rem + var(--gap-xl)) 0 var(--gap-xl);
    z-index: 3;
    text-align: left;

    .row {
        --gap: var(--gap-md);

        // 100vw 在桌面 Firefox（经典滚动条）下包含滚动条宽度会横向溢出，改用容器宽度
        width: 100%;
        gap: var(--gap);
        margin-bottom: var(--gap);
        display: flex;
        overflow: hidden;
        user-select: none;

        &:hover {
            .row__content {
                animation-play-state: paused !important;
            }
        }

        .row__content {
            flex-shrink: 0;
            display: flex;
            min-width: 100%;
            gap: var(--gap);
            animation: scroll 40s linear infinite;

            @media (prefers-reduced-motion: reduce) {
                animation-play-state: paused !important;
            }

            @keyframes scroll {
                from {
                    transform: translateX(0);
                }

                to {
                    transform: translateX(calc(-100%));
                }
            }

            &.offset {
                transform: translateX(-100%);
                animation: scroll-inverse 40s linear infinite;

                @keyframes scroll-inverse {
                    from {
                        transform: translateX(calc(-100%));
                    }

                    to {
                        transform: translateX(0);
                    }
                }
            }
        }

        .project {
            position: relative;
            display: flex;

            cursor: default;
            padding: 1rem;
            gap: 1rem;
            border-radius: 1rem;
            border: 1px solid var(--landing-border-color);
            transition:
                background 0.5s ease-in-out,
                transform 0.05s ease-in-out;
            // hover 位移在移动端卡顿，与 Modrinth 原版一致只保留背景渐变
            background: var(--landing-blob-gradient);
            user-select: none;

            &:hover {
                background: var(--landing-hover-card-gradient);
            }

            img {
                height: 3rem;
            }

            .project-info {
                box-sizing: border-box;
            }

            .title {
                color: var(--landing-color-heading);
                max-width: 13.75rem;
                overflow: hidden;
                white-space: nowrap;
                text-overflow: ellipsis;
                margin: 0;
                font-weight: 600;
                font-size: 1.25rem;
                line-height: 110%;
                display: block;
            }

            .description {
                width: 13.75rem;

                display: -webkit-box;
                -webkit-line-clamp: 2;
                -webkit-box-orient: vertical;
                overflow: hidden;

                font-weight: 500;
                font-size: 0.875rem;
                line-height: 125%;
                margin: 0.25rem 0 0;
            }
        }
    }

    .website-logo {
        position: absolute;
        top: 1.5rem;
        left: 50%;
        width: 3.5rem;
        height: 3.5rem;
        transform: translateX(-50%);
        z-index: 4;
        object-fit: contain;
    }

    h3 {
        font-weight: 500;
        font-size: var(--font-size-xl);
        color: var(--landing-color-heading);
        margin-bottom: 0.375rem;
        text-align: center;
    }

    p {
        font-size: var(--font-size-md);
        color: var(--landing-color-subheading);
        padding: var(--gap-xl);
        padding-top: 0;
        text-align: center;
    }
}

// 亮色模式项目卡背景（与页面其它卡片一致）
:global(html.light-mode) .project {
    background: rgba(255, 255, 255, 0.8) !important;
}
</style>
