import { loadGltf } from '../loaders/gltf-loader';
import type { AssetCatalog, AssetCatalogBuilder, AssetInfo } from './catalog';
import { type ModelSet, toModelSet } from './model-set';

// Loads assets by logical name through an AssetCatalog and decodes them into a ModelSet.
// The single place the glTF format is named; consumers see only ModelSet.
export class AssetStore {
    private catalog?: Promise<AssetCatalog>;
    private readonly cache = new Map<string, Promise<ModelSet>>();

    // Takes a builder so construction is free — the catalog manifest is fetched on first use.
    constructor(private readonly catalogBuilder: AssetCatalogBuilder) {}

    async list(): Promise<AssetInfo[]> {
        return (await this.getCatalog()).list();
    }

    // Rejects on failure (unknown name, fetch or decode error); a failed load is evicted
    // so a later call can retry rather than replay the cached rejection.
    loadModelSet(name: string): Promise<ModelSet> {
        let pending = this.cache.get(name);
        if (!pending) {
            pending = this.decodeModelSet(name);
            this.cache.set(name, pending);
            pending.catch(() => {
                if (this.cache.get(name) === pending) this.cache.delete(name);
            });
        }
        return pending;
    }

    // Loads a ModelSet from an arbitrary URL, bypassing the catalog. Uncached — for
    // ad-hoc sources such as a user-picked file. Rejects on failure.
    async loadModelSetFromUrl(url: string): Promise<ModelSet> {
        return toModelSet(await loadGltf(url), 'owned');
    }

    dispose(): void {
        this.cache.clear();
    }

    private getCatalog(): Promise<AssetCatalog> {
        return (this.catalog ??= this.catalogBuilder());
    }

    private async decodeModelSet(name: string): Promise<ModelSet> {
        const catalog = await this.getCatalog();
        return toModelSet(await loadGltf(catalog.url(name)), 'shared');
    }
}
