import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'
import { wireValidators } from './scripts/wire-validators.mjs'

// base relativa: o mesmo bundle serve a PWA, a casca e qualquer subcaminho.
export default defineConfig({
  base: './',
  plugins: [react(), wireValidators()],
  build: { target: 'es2022' },
  test: {
    environment: 'jsdom',
    css: { include: [/theme.css/] },
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
  },
})
