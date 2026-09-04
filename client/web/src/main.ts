import type { AssetCatalog } from './engine/assets/catalog';
import { createRoutedScene } from './index';

// Local-dev catalog for the shine-assets bucket. Lives only in the standalone entry, so
// it stays out of the library bundle a host consumes — that host injects its own catalog
// with a server-resolved version.
const ASSET_URL = 'https://assets.local.scytta.com:8093';
const ASSET_PLATFORM = 'web';
const ASSET_MODULE = 'models';

type Manifest = Record<string, string>;

// Resolves the shine-assets manifest up front so url() is synchronous afterwards.
// Protocol: latest.json -> version, {version}/{platform}/{module}/assets.json ->
// name -> relative blob path, blob at {baseUrl}/{relativeBlobPath}.
async function buildDefaultCatalog(): Promise<AssetCatalog> {
    const base = ASSET_URL.replace(/\/$/, '');
    const version = (await fetchJson<{ version: string }>(`${base}/latest.json`)).version;
    const manifest = await fetchJson<Manifest>(`${base}/${version}/${ASSET_PLATFORM}/${ASSET_MODULE}/assets.json`);

    return {
        list: () => Object.keys(manifest).map((name) => ({ name })),
        url: (name) => {
            const path = manifest[name];
            if (path === undefined) throw new Error(`[AssetCatalog] unknown asset "${name}"`);
            return `${base}/${path}`;
        }
    };
}

async function fetchJson<T>(url: string): Promise<T> {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`[AssetCatalog] failed to fetch ${url}: ${res.status}`);
    return (await res.json()) as T;
}

const container = document.getElementById('app')!;

// Standalone dev host, standing in for a real embedder: a rejection means the scene failed to
// start, onError means it died after starting. Both land here; a real host would render its own UI.
function showFatal(error: unknown): void {
    console.error('[shine-web] scene error:', error);
    container.replaceChildren();
    const box = document.createElement('div');
    box.textContent = describe(error);
    box.style.cssText = `
        position: absolute; inset: 0; overflow: auto;
        box-sizing: border-box; padding: 1.5rem;
        background: #101014; color: #ff9a9a;
        font: 13px/1.6 ui-monospace, Consolas, monospace;
        white-space: pre-wrap; word-break: break-word;
        user-select: text;
    `;
    container.appendChild(box);
}

// Full stack plus the cause chain, so the actual throw site is visible without opening the console.
function describe(error: unknown): string {
    if (!(error instanceof Error)) return String(error);
    let text = error.stack ?? `${error.name}: ${error.message}`;
    let cause: unknown = error.cause;
    while (cause !== undefined && cause !== null) {
        text += `\n\nCaused by: ${cause instanceof Error ? (cause.stack ?? cause.message) : String(cause)}`;
        cause = cause instanceof Error ? cause.cause : undefined;
    }
    return text;
}

// Anything escaping the runtime's error boundary (listeners, timers, foreign promises) would
// otherwise only show up in the console; surface it in the page too.
window.addEventListener('error', (event) => {
    showFatal(event.error ?? event.message);
});
window.addEventListener('unhandledrejection', (event) => {
    showFatal(event.reason);
});

void createRoutedScene(container, buildDefaultCatalog, showFatal).catch(showFatal);
