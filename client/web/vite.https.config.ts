import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig, mergeConfig } from 'vite';
import base from './vite.config';

// Local dev over https, so the page origin is accepted by the builder (https://*.local.scytta.com).
export default defineConfig((env) =>
    mergeConfig(typeof base === 'function' ? base(env) : base, {
        server: {
            host: true,
            port: 5173,
            https: {
                cert: readFileSync(fileURLToPath(new URL('../../certificates/scytta.crt', import.meta.url))),
                key: readFileSync(fileURLToPath(new URL('../../certificates/scytta.key', import.meta.url)))
            }
        }
    })
);
