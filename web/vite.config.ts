import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [vue()],
  server: {
    // 开发时把 /api 打到后端，省得开跨站白名单
    proxy: {
      '/api': 'http://127.0.0.1:51616',
    },
  },
  test: {
    environment: 'happy-dom',
    globals: true,
  },
})
