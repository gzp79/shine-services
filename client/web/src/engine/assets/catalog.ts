import type { ModelSet } from './model-set';

export interface AssetInfo {
    name: string;
}

export interface AssetCatalog {
    list(): AssetInfo[];
    url(name: string): string;
    generate?(name: string): Promise<ModelSet | undefined>;
}

// Deferred catalog construction. The caller of createScene supplies one: a host provides
// its own, dev standalone uses the local builder.
export type AssetCatalogBuilder = () => Promise<AssetCatalog>;
