import { createApp } from 'vue'

import App from './app/App.vue'
import Preview from './Preview.vue'
import { initTheme } from './themes'

// 主题要在挂载前定下来，否则会先闪一下没有配色的界面
initTheme()

// 原件预览留着当回归检查用，访问 #preview 就能看
const root = location.hash === '#preview' ? Preview : App

createApp(root).mount('#app')
