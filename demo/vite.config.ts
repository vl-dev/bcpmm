import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@cbmm/js-client': path.resolve(__dirname, '../sdk/js-client')
    }
  }
})

