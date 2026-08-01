import { createApp } from 'vue'

import Preview from './Preview.vue'
import { initTheme } from './themes'

// 主题要在挂载前定下来，否则会先闪一下没有配色的界面
initTheme()

createApp(Preview).mount('#app')
