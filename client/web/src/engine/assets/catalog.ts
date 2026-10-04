import type { ModelSet } from './model-set';

export const ASSET_KINDS = ['model', 'tile-3d', 'texture-ui'] as const;
export type AssetKind = (typeof ASSET_KINDS)[number];

export interface AssetInfo {
    name: string;
    kind: AssetKind;
}

export interface AssetCatalog {
    list(): AssetInfo[];
    url(name: string): string;
    generate?(name: string): Promise<ModelSet | undefined>;
}

// Deferred catalog construction. The caller of createScene supplies one: a host provides
// its own, dev standalone uses the local builder.
export type AssetCatalogBuilder = () => Promise<AssetCatalog>;
