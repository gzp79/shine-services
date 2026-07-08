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

export type VariantDef = {
    parts: SubMeshDef[];
};

export type InstancedMultiMeshParams = {
    geometry: THREE.BufferGeometry;
    variants: VariantDef[];
    instanceCountHint?: number;
};

export type InstanceBufferLayout = {
    buffers: Array<{ floatsPerInstance: number; isDynamic?: boolean }>;
};

const DEFAULT_INSTANCE_HINT = 1024;

class SubMesh extends THREE.Mesh {
    constructor(
        sourceGeo: THREE.BufferGeometry,
        indexStart: number,
        indexEnd: number,
        readonly instanceBuffer: InstanceBuffer,
        material: THREE.Material
    ) {
        const geo = new THREE.InstancedBufferGeometry();
        for (const [name, attr] of Object.entries(sourceGeo.attributes)) geo.setAttribute(name, attr);
        geo.setIndex(sourceGeo.index);
        geo.setDrawRange(indexStart, indexEnd - indexStart);
        geo.instanceCount = 0;

        super(geo, material);
        this.instanceBuffer = instanceBuffer;
        this.frustumCulled = false;
    }
}

type VariantEntry = {
    instanceBuffer: InstanceBuffer;
    instanceData: InstanceData;
    subMeshes: SubMesh[];
    parts: SubMeshDef[];
};

export abstract class InstancedMultiMesh {
    readonly group = new THREE.Group();
    protected readonly sourceGeo: THREE.BufferGeometry;
    private readonly variants: VariantEntry[] = [];

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

        for (const variantDef of params.variants) {
            const instanceBuffer = new InstanceBuffer(hint, texelsPerBuffer);
            const instanceData = new InstanceData(instanceBuffer.textures);
            const subMeshes: SubMesh[] = [];
            const entry: VariantEntry = { instanceBuffer, instanceData, subMeshes, parts: variantDef.parts };
            this.variants.push(entry);

            for (let pi = 0; pi < variantDef.parts.length; pi++) {
                const part = variantDef.parts[pi];
                const mat = this.createMaterial(part.materialName, instanceData);
                const mesh = new SubMesh(this.sourceGeo, part.indexStart, part.indexEnd, instanceBuffer, mat);
                this.group.add(mesh);
                subMeshes.push(mesh);

                const isPrimary = pi === 0;
                mesh.onBeforeRender = (_renderer, _scene, _camera, geometry) => {
                    if (isPrimary) {
                        if (entry.instanceBuffer.compact()) this._onGrow(entry);
                        for (let i = 0; i < entry.instanceBuffer.textures.length; i++) {
                            if (entry.instanceBuffer.isDirty(i)) {
                                entry.instanceBuffer.textures[i].needsUpdate = true;
                                entry.instanceBuffer.clearDirty(i);
                            }
                        }
                    }
                    (geometry as THREE.InstancedBufferGeometry).instanceCount = entry.instanceBuffer.length;
                };
            }
        }
    }

    protected abstract instanceBufferLayout(): InstanceBufferLayout;
    protected abstract createMaterial(materialName: string, instanceData: InstanceData): THREE.Material;

    private _onGrow(entry: VariantEntry): void {
        console.log(`[InstancedMultiMesh] grow → capacity=${entry.instanceBuffer.maxInstances}`);
        entry.instanceData.replaceTextures(entry.instanceBuffer.textures);
        for (let pi = 0; pi < entry.subMeshes.length; pi++) {
            const mesh = entry.subMeshes[pi];
            const oldMat = mesh.material as THREE.Material;
            mesh.material = this.createMaterial(entry.parts[pi].materialName, entry.instanceData);
            oldMat.dispose();
        }
    }

    protected setInstance(variantIndex: number, key: number, bufIndex: number, values: Float32Array): boolean {
        const entry = this.variants[variantIndex];
        if (!entry) return false;
        return entry.instanceBuffer.setBuffer(key, bufIndex, values);
    }

    removeInstance(variantIndex: number, key: number): boolean {
        return this.variants[variantIndex]?.instanceBuffer.remove(key) ?? false;
    }

    instanceCount(variantIndex: number): number {
        return this.variants[variantIndex]?.instanceBuffer.length ?? 0;
    }

    instances(variantIndex: number): IterableIterator<number> {
        return this.variants[variantIndex]?.instanceBuffer.keys ?? [][Symbol.iterator]();
    }

    dispose(): void {
        for (const entry of this.variants) {
            entry.instanceBuffer.dispose();
            for (const mesh of entry.subMeshes) {
                (mesh.material as THREE.Material).dispose();
                mesh.geometry.dispose();
            }
        }
        this.variants.length = 0;
        this.group.parent?.remove(this.group);
    }
}
