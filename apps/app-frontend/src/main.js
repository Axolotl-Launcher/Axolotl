import 'overlayscrollbars/overlayscrollbars.css'
import '@modrinth/ui/directives/tooltip-styles.ts'

import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import { createApp } from 'vue'

import App from '@/App.vue'
import { overlayScrollbarsDirective } from '@/directives/overlayScrollbars'
import { installTelemetryHandlers } from '@/helpers/telemetry'
import { initializeLocale } from '@/i18n.config'
import i18nPlugin from '@/plugins/i18n'
import i18nDebugPlugin from '@/plugins/i18n-debug'
import router from '@/routes'

const pinia = createPinia()

const app = createApp(App)

installTelemetryHandlers()

app.use(VueQueryPlugin)
// Pinia must install before the router: route guards use useNavigationReturnStore().
app.use(pinia)
app.use(router)
app.use(i18nPlugin)
app.use(i18nDebugPlugin)
app.directive('overlay-scrollbars', overlayScrollbarsDirective)
app.directive('tooltip', tooltipDirective)

app.mount('#app')
void initializeLocale().catch(console.error)
