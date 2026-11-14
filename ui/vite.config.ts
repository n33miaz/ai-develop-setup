import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// base relativa: o mesmo bundle serve a PWA, a casca e qualquer subcaminho.
export default defineConfig({
  base: './',
  plugins: [react()],
  build: { target: 'es2022' },
  test: {
    environment: 'jsdom',
    css: { include: [/theme.css/] },
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
  },
})
