<script setup lang="ts">
import { useVIntl } from '@modrinth/ui'

import LegalDocument from '~/components/ui/LegalDocument.vue'

const { locale } = useVIntl()
const isChinese = computed(() => locale.value === 'zh-CN')

const seoTitle = computed(() =>
    isChinese.value ? '隐私政策 - Axolotl Launcher' : 'Privacy Policy - Axolotl Launcher',
)
const seoDescription = computed(() =>
    isChinese.value
        ? '了解 Axolotl Launcher 官方网站和桌面应用如何处理数据。'
        : 'Learn how the Axolotl Launcher official website and desktop application handle data.',
)

useSeoMeta({
    title: () => seoTitle.value,
    description: () => seoDescription.value,
    ogTitle: () => seoTitle.value,
    ogDescription: () => seoDescription.value,
    ogType: 'website',
    ogUrl: 'https://axlmc.org/privacy',
    robots: 'index, follow',
})

useHead({
    link: [{ rel: 'canonical', href: 'https://axlmc.org/privacy' }],
})
</script>

<template>
    <LegalDocument
        v-if="isChinese"
        :key="`zh-${locale}`"
        eyebrow="隐私政策"
        title="隐私政策"
        description="本政策说明 Axolotl Launcher 官方网站和桌面应用处理数据的方式，以及第三方服务可能接收的信息。"
        updated-at="2026 年 8 月 14 日"
    >
        <h2>1. 适用范围</h2>
        <p>
            本政策适用于 www.axlmc.org 官方网站和 Axolotl Launcher
            桌面应用。通过启动器访问的第三方网站、API、认证服务、服务器及内容下载地址，适用对应第三方自己的隐私政策。
        </p>

        <h2>2. 官方网站处理的数据</h2>
        <p>网站无需注册账户。为保存你的显示和下载偏好，浏览器会在本地保存：</p>
        <ul>
            <li>语言偏好 Cookie（<code>locale</code>）；</li>
            <li>主题、高级渲染、减少动态效果和外部链接打开方式；</li>
            <li>下载源选择（自动、Update Server 或 GitHub）。</li>
        </ul>
        <p>
            这些偏好主要保存在 Cookie 或浏览器 localStorage
            中，你可通过浏览器设置清除。网站不使用广告 Cookie。网站接入<a
                href="https://howxm.com"
                target="_blank"
                rel="noopener"
                >浩客（Howxm）</a
            >
            作为第三方用户体验分析和反馈收集服务；其可根据服务配置处理与网站访问、交互、设备或浏览器及网络请求相关的信息，以展示问卷、收集评价和改进网站体验。
        </p>
        <p>
            网站托管和网络基础设施提供者可能为保障访问、安全和故障排查而处理常规请求信息，例如 IP
            地址、访问时间、请求路径、User-Agent 和错误日志。此类处理受对应提供者政策约束。
        </p>

        <h2>3. 桌面应用处理的数据</h2>
        <p>启动器会在你的设备本地处理并保存运行所需的数据，包括：</p>
        <ul>
            <li>应用设置、界面偏好和下载配置；</li>
            <li>Minecraft 实例、游戏版本、模组及其他内容的元数据；</li>
            <li>账户资料、登录会话、离线账户信息和自定义认证服务地址；</li>
            <li>游戏日志、崩溃信息、缓存、下载记录及更新状态。</li>
        </ul>
        <p>
            这些数据用于登录、启动游戏、管理内容、诊断故障和提供你主动使用的功能。启动器会在首次运行或功能升级后询问是否允许匿名遥测；未确认或关闭时不会发送遥测。允许后，启动器每天最多发送一次匿名活跃信号，用于统计选择加入遥测的匿名安装量、DAU、WAU、MAU。不会自动上传完整的启动器日志或
            Minecraft 令牌、账户凭据或原始随机安装标识。
        </p>
        <p>
            遥测使用本地生成的随机安装标识；服务端仅保存经密钥 HMAC-SHA256
            处理后的不可逆标识。你可随时在“隐私与安全”中关闭遥测；关闭后会停止后续采集并清空本地待发送队列，但已经上传的数据仍按下述保留周期清理。Discord
            Rich Presence 是独立的本地开关，不属于遥测。
        </p>

        <h2>4. 与第三方共享和传输</h2>
        <p>
            当你使用登录、搜索、下载、更新、皮肤或联机等功能时，启动器会按你的操作直接连接第三方服务。请求可能包含服务完成操作所需的账户令牌、用户标识、项目或文件标识、IP
            地址、设备和网络请求信息。可能涉及的服务包括：
        </p>
        <ul>
            <li>Microsoft 和 Minecraft 的登录、资料及游戏服务；</li>
            <li>你选择的第三方 Yggdrasil 认证服务；</li>
            <li>Modrinth、CurseForge 及内容作者提供的 API 和下载地址；</li>
            <li>Update Server、GitHub 及其他用于版本检查、更新或文件分发的服务；</li>
            <li>用于官网用户体验分析和反馈收集的浩客（Howxm）；</li>
            <li>用于接收和保存选择加入的匿名遥测数据的 Cloudflare Workers 和 D1；</li>
            <li>你主动连接的 Minecraft 服务器或其他外部链接。</li>
        </ul>
        <p>
            项目维护者不会出售你的个人信息。第三方如何保存和使用请求数据，由其各自的隐私政策决定。
        </p>

        <h2>5. 数据保存与删除</h2>
        <p>
            网站偏好保存在你的浏览器中；启动器数据主要保存在你的设备中。你可以清除浏览器站点数据，或在启动器内移除账户、实例和缓存。卸载应用不一定自动删除全部数据目录，请在确认备份需求后手动清理残留文件。
        </p>
        <p>
            遥测中的匿名日活记录保留 35
            天；每日汇总和匿名安装哈希会长期保留，用于历史趋势和累计安装统计。项目不会在遥测数据库中保存请求
            IP。Cloudflare 作为基础设施提供者仍可能按其自身政策处理网络请求所必需的信息。
        </p>
        <p>
            第三方服务保存的数据须通过对应服务的账户设置或隐私渠道管理。项目维护者无法代你访问、导出或删除第三方持有的数据。
        </p>

        <h2>6. 数据安全</h2>
        <p>
            我们通过开源审查、系统权限和安全更新等方式降低风险，但无法保证绝对安全。请从官方渠道下载软件、及时更新、保护系统账户，不要公开登录令牌、完整日志或包含个人信息的截图。
        </p>

        <h2>7. 未成年人</h2>
        <p>
            未成年人应在监护人指导下使用本项目，并遵守所在地区的年龄要求及 Minecraft、Microsoft
            和其他相关服务的规则。请勿通过公开反馈渠道提交不必要的个人信息。
        </p>

        <h2>8. 政策变更</h2>
        <p>
            本政策可能因功能、第三方服务或法律要求变化而更新。新版本将在本页面发布，并标明最后更新日期。重大变化会尽量通过官方网站、GitHub
            仓库或版本说明告知。
        </p>

        <h2>9. 联系方式</h2>
        <p>
            如有隐私问题或发现安全风险，请通过
            <a href="https://github.com/Mystic-Stars/Axolotl/issues" target="_blank" rel="noopener"
                >GitHub Issues</a
            >
            联系项目维护者。请勿在公开 Issue
            中提交账户令牌、密码、完整日志或其他敏感信息。也可加入官方 QQ 群 830619067
            咨询一般问题。
        </p>
    </LegalDocument>
    <LegalDocument
        v-else
        :key="`en-${locale}`"
        eyebrow="Privacy Policy"
        title="Privacy Policy"
        description="This policy explains how the Axolotl Launcher official website and desktop application handle data, and what information third-party services may receive."
        updated-at="August 14, 2026"
    >
        <h2>1. Scope</h2>
        <p>
            This policy applies to the www.axlmc.org official website and the Axolotl Launcher
            desktop application. Third-party websites, APIs, authentication services, servers, and
            content download addresses accessed through the launcher are governed by their own
            privacy policies.
        </p>

        <h2>2. Data Processed by the Official Website</h2>
        <p>
            The website does not require an account. To remember your display and download
            preferences, your browser stores locally:
        </p>
        <ul>
            <li>A language preference cookie (<code>locale</code>);</li>
            <li>Theme, advanced rendering, reduced motion, and external-link behavior;</li>
            <li>Your download-source selection (automatic, Update Server, or GitHub).</li>
        </ul>
        <p>
            These preferences are mainly stored in cookies or browser localStorage and can be
            cleared through your browser settings. The website uses no advertising cookies. It
            integrates
            <a href="https://howxm.com" target="_blank" rel="noopener">Howxm</a> as a third-party
            user-experience analytics and feedback collection service. Depending on its service
            configuration, Howxm may process information related to website visits, interactions,
            devices or browsers, and network requests to display surveys, collect ratings, and
            improve the website experience.
        </p>
        <p>
            Website hosting and network infrastructure providers may process routine request
            information such as IP addresses, access times, request paths, User-Agent strings, and
            error logs to ensure availability, security, and troubleshooting. Such processing is
            subject to the corresponding providers' policies.
        </p>

        <h2>3. Data Processed by the Desktop Application</h2>
        <p>
            The launcher processes and stores the data needed to run locally on your device,
            including:
        </p>
        <ul>
            <li>Application settings, interface preferences, and download configuration;</li>
            <li>Metadata for Minecraft instances, game versions, mods, and other content;</li>
            <li>
                Account profiles, login sessions, offline account information, and custom
                authentication service addresses;
            </li>
            <li>Game logs, crash information, caches, download records, and update status.</li>
        </ul>
        <p>
            This data is used for signing in, launching the game, managing content, diagnosing
            faults, and providing features you actively use. The launcher asks whether to allow
            anonymous telemetry on first use or after a relevant consent update. No telemetry is
            sent before confirmation or while it is disabled. When enabled, the launcher sends at
            most one anonymous daily activity signal. This is used to count opted-in anonymous
            installations, DAU, WAU, and MAU. Full launcher or Minecraft logs, Minecraft tokens,
            account credentials, and the original random installation identifier are not uploaded
            automatically.
        </p>
        <p>
            Telemetry uses a random installation identifier generated locally; the service stores
            only an irreversible HMAC-SHA256 value produced with a server secret. You can disable
            telemetry at any time under Privacy & security. Disabling it stops future collection and
            clears the local pending queue, while previously uploaded records are deleted under the
            retention periods below. Discord Rich Presence is an independent local setting and is
            not telemetry.
        </p>

        <h2>4. Sharing and Transfers with Third Parties</h2>
        <p>
            When you use features such as sign-in, search, download, update, skins, or multiplayer,
            the launcher connects directly to third-party services based on your actions. Requests
            may include the account tokens, user identifiers, project or file identifiers, IP
            addresses, device, and network request information needed to complete the operation.
            Services that may be involved include:
        </p>
        <ul>
            <li>Microsoft and Minecraft sign-in, profile, and game services;</li>
            <li>Third-party Yggdrasil authentication services you choose;</li>
            <li>
                APIs and download addresses provided by Modrinth, CurseForge, and content authors;
            </li>
            <li>
                Update Server, GitHub, and other services used for version checks, updates, or file
                distribution;
            </li>
            <li>Howxm for official-website user-experience analytics and feedback collection;</li>
            <li>
                Cloudflare Workers and D1 for opted-in anonymous telemetry ingestion and storage;
            </li>
            <li>Minecraft servers or other external links you connect to.</li>
        </ul>
        <p>
            The project maintainers do not sell your personal information. How third parties store
            and use request data is determined by their respective privacy policies.
        </p>

        <h2>5. Data Retention and Deletion</h2>
        <p>
            Website preferences are stored in your browser; launcher data is mainly stored on your
            device. You can clear browser site data or remove accounts, instances, and caches inside
            the launcher. Uninstalling the application does not necessarily delete all data
            directories automatically; after confirming your backup needs, manually clean up any
            leftover files.
        </p>
        <p>
            Anonymous daily-active records are retained for 35 days; daily totals and anonymous
            installation hashes are retained long-term for historical trends and cumulative
            installation counts. The project does not store request IP addresses in the telemetry
            database. Cloudflare, as the infrastructure provider, may still process information
            required to deliver network requests under its own policies.
        </p>
        <p>
            Data stored by third-party services must be managed through the corresponding service's
            account settings or privacy channels. The project maintainers cannot access, export, or
            delete data held by third parties on your behalf.
        </p>

        <h2>6. Data Security</h2>
        <p>
            We reduce risk through open-source review, system permissions, and security updates, but
            cannot guarantee absolute security. Download software from official channels, update
            regularly, protect your system accounts, and do not publicly share login tokens, full
            logs, or screenshots containing personal information.
        </p>

        <h2>7. Minors</h2>
        <p>
            Minors should use this project under the guidance of a guardian and comply with the age
            requirements of their region as well as the rules of Minecraft, Microsoft, and other
            related services. Do not submit unnecessary personal information through public feedback
            channels.
        </p>

        <h2>8. Changes to This Policy</h2>
        <p>
            This policy may be updated as features, third-party services, or legal requirements
            change. New versions will be published on this page with a last-updated date.
            Significant changes will be announced through the official website, the GitHub
            repository, or release notes when possible.
        </p>

        <h2>9. Contact</h2>
        <p>
            For privacy questions or security concerns, contact the project maintainers via
            <a href="https://github.com/Mystic-Stars/Axolotl/issues" target="_blank" rel="noopener"
                >GitHub Issues</a
            >. Do not submit account tokens, passwords, full logs, or other sensitive information in
            public issues. You may also join the official QQ group 830619067 for general questions.
        </p>
    </LegalDocument>
</template>
