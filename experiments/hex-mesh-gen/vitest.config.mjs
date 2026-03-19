import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['tools/hex-mesh-gen/__tests__/**/*.test.mjs'],
  },
});
