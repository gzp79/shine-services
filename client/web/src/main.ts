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
void createRoutedScene(container, buildDefaultCatalog);
