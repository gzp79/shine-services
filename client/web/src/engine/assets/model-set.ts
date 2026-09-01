import type * as THREE from 'three';
import type { MeshStandardNodeMaterial } from 'three/webgpu';
import type { GltfAsset } from '../loaders/gltf-loader';
import { type Shareable, own, share } from '../resources/ownership';

// Resources are shared by default: the AssetStore owns them and consumers only borrow. When decoded
// for a single consumer they are owned instead, and that consumer disposes them.
export interface ModelPart {
    material: Shareable<MeshStandardNodeMaterial>;
    indexStart: number;
    indexEnd: number;
}

// A selectable sub-model packed into the shared geometry (a viewer mesh / an instancing variant).
export interface ModelEntry {
    name: string;
    parts: ModelPart[];
}

// Format-neutral result of loading an asset. Carries no trace of the transport format.
export interface ModelSet {
    geometry: Shareable<THREE.BufferGeometry>;
    models: ModelEntry[];
}

// Maps a decoded glTF into the neutral ModelSet — the single place the format is erased.
export function toModelSet(asset: GltfAsset, ownership: 'shared' | 'owned'): ModelSet {
    const mark = ownership === 'owned' ? own : share;
    return {
        geometry: mark(asset.geometry),
        models: asset.meshes.map((m) => ({
            name: m.name,
            parts: m.submeshes.map((s) => ({
                material: mark(s.material),
                indexStart: s.indexStart,
                indexEnd: s.indexEnd
            }))
        }))
    };
}
