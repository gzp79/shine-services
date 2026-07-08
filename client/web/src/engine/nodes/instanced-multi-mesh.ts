import * as THREE from 'three';
import { instanceIndex, ivec2, textureLoad, vec2, vec3, vec4 } from 'three/tsl';
import { InstanceBuffer, nextPow2 } from './instance-buffer';

const SWIZZLE = ['x', 'y', 'z', 'w'] as const;

export class InstanceData {
    private textures: readonly THREE.DataTexture[];
    private readonly cache = new Map<number, ReturnType<typeof textureLoad>>();
    private readonly iIdx = instanceIndex.toInt();

    constructor(textures: readonly THREE.DataTexture[]) {
        this.textures = textures;
    }

    replaceTextures(newTextures: readonly THREE.DataTexture[]): void {
        this.textures = newTextures;
        this.cache.clear();
    }

    private texel(bufIdx: number, row: number): ReturnType<typeof textureLoad> {
        const key = bufIdx * 1024 + row;
        if (!this.cache.has(key)) this.cache.set(key, textureLoad(this.textures[bufIdx], ivec2(row, this.iIdx)));
        return this.cache.get(key)!;
    }

    private float(bufIdx: number, fi: number) {
        return this.texel(bufIdx, Math.floor(fi / 4))[SWIZZLE[fi % 4]];
    }

    vec2(bufIdx: number, i: number) {
        const s = i * 2;
        return vec2(this.float(bufIdx, s), this.float(bufIdx, s + 1));
    }

    vec3(bufIdx: number, i: number) {
        const s = i * 3;
        return vec3(this.float(bufIdx, s), this.float(bufIdx, s + 1), this.float(bufIdx, s + 2));
    }

    vec4(bufIdx: number, i: number) {
        const s = i * 4;
        return vec4(
            this.float(bufIdx, s),
            this.float(bufIdx, s + 1),
            this.float(bufIdx, s + 2),
            this.float(bufIdx, s + 3)
        );
    }
}

export type SubMeshDef = {
    materialName: string;
    indexStart: number;
    indexEnd: number;
};

export type TileTypeDef = {
    parts: SubMeshDef[];
};

export type InstancedMultiMeshParams = {
    geometry: THREE.BufferGeometry;
    tileTypes: TileTypeDef[];
    instanceCountHint?: number;
};

export type InstanceBufferLayout = {
    buffers: Array<{ floatsPerInstance: number; isDynamic?: boolean }>;
};

const DEFAULT_INSTANCE_HINT = 1024;

class SubMesh extends THREE.Mesh {
    readonly instanceBuffer: InstanceBuffer;
    readonly isPrimary: boolean;

    constructor(
        sourceGeo: THREE.BufferGeometry,
        indexStart: number,
        indexEnd: number,
        instanceBuffer: InstanceBuffer,
        material: THREE.Material,
        isPrimary: boolean
    ) {
        const geo = new THREE.InstancedBufferGeometry();
        for (const [name, attr] of Object.entries(sourceGeo.attributes)) geo.setAttribute(name, attr);
        geo.setIndex(sourceGeo.index);
        geo.setDrawRange(indexStart, indexEnd - indexStart);
        geo.instanceCount = 0;

        super(geo, material);
        this.instanceBuffer = instanceBuffer;
        this.isPrimary = isPrimary;
        this.frustumCulled = false;
        this.onBeforeRender = (_renderer, _scene, _camera, geometry, _material, _group) => {
            const count = this.isPrimary ? this.instanceBuffer.compact() : this.instanceBuffer.length;
            (geometry as THREE.InstancedBufferGeometry).instanceCount = count;
            for (let i = 0; i < this.instanceBuffer.textures.length; i++) {
                if (this.instanceBuffer.isDirty(i)) {
                    this.instanceBuffer.textures[i].needsUpdate = true;
                    this.instanceBuffer.clearDirty(i);
                }
            }
        };
    }
}

type TileTypeEntry = {
    instanceBuffer: InstanceBuffer;
    instanceData: InstanceData;
    subMeshes: SubMesh[];
};

export abstract class InstancedMultiMesh {
    readonly group = new THREE.Group();
    protected readonly sourceGeo: THREE.BufferGeometry;
    private readonly entries: TileTypeEntry[] = [];

    protected constructor(parent: THREE.Object3D, params: InstancedMultiMeshParams) {
        parent.add(this.group);
        this.sourceGeo = params.geometry;
        const layout = this.instanceBufferLayout();
        const hint = nextPow2(Math.max(1, params.instanceCountHint ?? DEFAULT_INSTANCE_HINT));

        const texelsPerBuffer = layout.buffers.map((b) => {
            if (b.floatsPerInstance % 4 !== 0)
                throw new Error(`floatsPerInstance (${b.floatsPerInstance}) must be a multiple of 4`);
            return b.floatsPerInstance / 4;
        });

        for (const tileDef of params.tileTypes) {
            const instanceBuffer = new InstanceBuffer(hint, texelsPerBuffer);
            const instanceData = new InstanceData(instanceBuffer.textures);
            const subMeshes: SubMesh[] = [];

            for (let pi = 0; pi < tileDef.parts.length; pi++) {
                const part = tileDef.parts[pi];
                const mat = this.createMaterial(part.materialName, instanceData);
                const mesh = new SubMesh(this.sourceGeo, part.indexStart, part.indexEnd, instanceBuffer, mat, pi === 0);
                this.group.add(mesh);
                subMeshes.push(mesh);
            }

            this.entries.push({ instanceBuffer, instanceData, subMeshes });
        }
    }

    protected abstract instanceBufferLayout(): InstanceBufferLayout;
    protected abstract createMaterial(materialName: string, instanceData: InstanceData): THREE.Material;

    protected setInstance(tileTypeIndex: number, key: number, bufIndex: number, values: Float32Array): boolean {
        const entry = this.entries[tileTypeIndex];
        if (!entry) return false;

        if (entry.instanceBuffer.length >= entry.instanceBuffer.maxInstances) {
            const newCap = entry.instanceBuffer.maxInstances * 2;
            const newTextures = entry.instanceBuffer.grow(newCap);
            entry.instanceData.replaceTextures(newTextures);
            for (const mesh of entry.subMeshes) {
                (mesh.material as THREE.Material).needsUpdate = true;
            }
        }

        return entry.instanceBuffer.setBuffer(key, bufIndex, values);
    }

    removeInstance(tileTypeIndex: number, key: number): boolean {
        return this.entries[tileTypeIndex]?.instanceBuffer.remove(key) ?? false;
    }

    instanceCount(tileTypeIndex: number): number {
        return this.entries[tileTypeIndex]?.instanceBuffer.length ?? 0;
    }

    instances(tileTypeIndex: number): IterableIterator<number> {
        return this.entries[tileTypeIndex]?.instanceBuffer.keys ?? [][Symbol.iterator]();
    }

    dispose(): void {
        for (const entry of this.entries) {
            entry.instanceBuffer.dispose();
            for (const mesh of entry.subMeshes) {
                (mesh.material as THREE.Material).dispose();
                mesh.geometry.dispose();
            }
        }
        this.entries.length = 0;
        this.group.parent?.remove(this.group);
    }
}
