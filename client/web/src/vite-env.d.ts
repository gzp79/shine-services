/// <reference types="vite/client" />

declare module '#wasm-bin' {
    const url: string;
    export default url;
}

declare global {
    interface ImportMeta {
        readonly env: ImportMetaEnv & {
            // True when the wasm was built with the `heap-profiling` feature; injected by vite.
            readonly VITE_HEAP_PROFILING: boolean;
        };
    }
}

export {};
