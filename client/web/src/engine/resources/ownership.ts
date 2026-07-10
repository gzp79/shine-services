import * as THREE from 'three';
import { MeshStandardNodeMaterial } from 'three/webgpu';

export type Owned<T> = T & { __shineOwnership: 'owned' };
export type Shared<T> = T & { __shineOwnership: 'shared' };
export type Shareable<T> = Owned<T> | Shared<T>;

export type OwnedBufferGeometry = Owned<THREE.BufferGeometry>;
export type SharedBufferGeometry = Shared<THREE.BufferGeometry>;

export type OwnedMaterial = Owned<THREE.Material>;
export type SharedMaterial = Shared<THREE.Material>;
export type SharedMeshStandardNodeMaterial = Shared<MeshStandardNodeMaterial>;

export function own<T extends object>(t: T): Owned<T> {
    return Object.assign(t, { __shineOwnership: 'owned' as const });
}

export function share<T extends object>(t: T): Shared<T> {
    return Object.assign(t, { __shineOwnership: 'shared' as const });
}

export function disposeIfOwned(resource: { __shineOwnership: 'owned' | 'shared'; dispose(): void }): void {
    if (resource.__shineOwnership === 'owned') resource.dispose();
}

export function disposeObject3D(obj: THREE.Object3D): void {
    obj.traverse((child) => {
        if ('dispose' in child && typeof (child as { dispose: unknown }).dispose === 'function') {
            (child as { dispose(): void }).dispose();
        }
    });
}
