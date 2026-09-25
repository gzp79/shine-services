import * as THREE from 'three';
import { instanceIndex, int, ivec2, textureLoad, vec2, vec3, vec4 } from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import { type OwnedMaterial, type Shareable, disposeIfOwned, own } from '../../resources/ownership';
import { cloneStandardMaterial } from '../../utils';
import { InstanceBuffer } from './instance-buffer';

const SWIZZLE = ['x', 'y', 'z', 'w'] as const;

export class InstanceData {
    readonly schema: symbol;

    private textures: readonly THREE.DataTexture[];
    private readonly cache = new Map<string, ReturnType<typeof textureLoad>>();
    private readonly iIdx = instanceIndex.toInt();
    private readonly texelsPerBuf: readonly number[] | null;
    private readonly stripHeight: number;

    constructor(
        textures: readonly THREE.DataTexture[],
        stripHeight: number,
        texelsPerBuf: readonly number[] | null,
        schema: symbol
    ) {
        this.textures = textures;
        this.stripHeight = stripHeight;
        this.texelsPerBuf = texelsPerBuf;
        this.schema = schema;
    }

    replaceTextures(newTextures: readonly THREE.DataTexture[]): void {
        this.textures = newTextures;
        this.cache.clear();
    }

    private texel(bufIdx: number, texelOffset: number): ReturnType<typeof textureLoad> {
        const key = `${bufIdx}:${texelOffset}`;
        const cached = this.cache.get(key);
        if (cached !== undefined) return cached;
        let coord;
        if (this.stripHeight > 0) {
            // strip layout: width = numStrips * T, height = stripHeight (fixed)
            // slot = iIdx; strip = floor(slot / stripHeight); row = slot % stripHeight
            // x = strip * T + texelOffset;  y = row
            const T = this.texelsPerBuf![bufIdx];
            const strip = this.iIdx.div(int(this.stripHeight));
            const row = this.iIdx.mod(int(this.stripHeight));
            coord = ivec2(strip.mul(int(T)).add(int(texelOffset)), row);
        } else {
            // tall layout: width = T, height = maxInstances
            // x = texelOffset, y = instanceIndex
            coord = ivec2(texelOffset, this.iIdx);
        }
        const result = textureLoad(this.textures[bufIdx], coord);
        this.cache.set(key, result);
        return result;
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

    vec3At(bufIdx: number, floatIndex: number) {
        return vec3(
            this.float(bufIdx, floatIndex),
            this.float(bufIdx, floatIndex + 1),
            this.float(bufIdx, floatIndex + 2)
        );
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
    baseMaterial: Shareable<MeshStandardNodeMaterial>;
    indexStart: number;
    indexEnd: number;
};

export type VariantDef = {
    parts: SubMeshDef[];
};

export type InstancedMultiMeshParams = {
    geometry: Shareable<THREE.BufferGeometry>;
    variants: VariantDef[];
    instanceCountHint?: number;
    // Strip height for wide-texture mode. When set, the texture grows in width
    // (adding strips) rather than height, keeping height <= pageSizeHint.
    // Defaults to 0 (tall texture mode, grows height).
    pageSizeHint?: number;
};

export type InstanceBufferLayout = {
    schema: symbol;
    buffers: Array<{ floatsPerInstance: number }>;
};

// Minimal lifecycle contract for an opt-in add-on (e.g. InstancedNormalLineAttachment) — see attach()/detach() below.
export interface InstancedMeshAttachment {
    attach(target: InstancedMultiMesh): void;
    instanceDataChanged(target: InstancedMultiMesh, variantIndex: number): void;
    dispose(): void;
}

const DEFAULT_INSTANCE_HINT = 1024;

class SubMesh extends THREE.Mesh {
    declare material: OwnedMaterial;

    constructor(
        sourceGeo: THREE.BufferGeometry,
        indexStart: number,
        indexEnd: number,
        readonly instanceBuffer: InstanceBuffer,
        material: OwnedMaterial
    ) {
        const geo = new THREE.InstancedBufferGeometry();
        for (const [name, attr] of Object.entries(sourceGeo.attributes)) geo.setAttribute(name, attr);
        geo.setIndex(sourceGeo.index);
        geo.setDrawRange(indexStart, indexEnd - indexStart);
        geo.instanceCount = 0;

        super(geo, material);
        this.frustumCulled = false;
    }

    replaceMaterial(material: OwnedMaterial): void {
        disposeIfOwned(this.material);
        this.material = material;
    }

    dispose(): void {
        disposeIfOwned(this.material);
        this.geometry.dispose();
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
    protected readonly sourceGeo: Shareable<THREE.BufferGeometry>;
    private readonly variants: VariantEntry[] = [];
    private readonly stripHeight: number;
    private readonly keyVariant = new Map<number, number>();
    private readonly attachments = new Map<string, InstancedMeshAttachment>();

    protected constructor(parent: THREE.Object3D, params: InstancedMultiMeshParams) {
        parent.add(this.group);
        this.sourceGeo = params.geometry;
        this.stripHeight = params.pageSizeHint ?? 0;
        const layout = this.instanceBufferLayout();
        const hint = Math.max(1, params.instanceCountHint ?? DEFAULT_INSTANCE_HINT);

        const texelsPerBuffer = layout.buffers.map((b) => {
            if (b.floatsPerInstance % 4 !== 0)
                throw new Error(`floatsPerInstance (${b.floatsPerInstance}) must be a multiple of 4`);
            return b.floatsPerInstance / 4;
        });

        params.variants.forEach((variantDef, variantIndex) => {
            const instanceBuffer = new InstanceBuffer(hint, texelsPerBuffer, this.stripHeight);
            const instanceData = this._makeInstanceData(instanceBuffer.textures, texelsPerBuffer, layout.schema);
            const subMeshes: SubMesh[] = [];
            const entry: VariantEntry = {
                instanceBuffer,
                instanceData,
                subMeshes,
                parts: variantDef.parts
            };
            this.variants.push(entry);

            for (let pi = 0; pi < variantDef.parts.length; pi++) {
                const part = variantDef.parts[pi];
                const mat = own(this.createMaterial(cloneStandardMaterial(part.baseMaterial), instanceData));
                const mesh = new SubMesh(this.sourceGeo, part.indexStart, part.indexEnd, instanceBuffer, mat);
                this.group.add(mesh);
                subMeshes.push(mesh);

                const isPrimary = pi === 0;
                mesh.onBeforeRender = (_renderer, _scene, _camera, geometry) => {
                    if (isPrimary) {
                        if (entry.instanceBuffer.compact()) this._onGrow(entry, variantIndex);
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
        });
    }

    private _makeInstanceData(
        textures: readonly THREE.DataTexture[],
        texelsPerBuffer: readonly number[],
        schema: symbol
    ): InstanceData {
        return new InstanceData(textures, this.stripHeight, this.stripHeight > 0 ? texelsPerBuffer : null, schema);
    }

    protected abstract instanceBufferLayout(): InstanceBufferLayout;
    protected abstract createMaterial(
        mat: MeshStandardNodeMaterial,
        instanceData: InstanceData
    ): MeshStandardNodeMaterial;

    private _onGrow(entry: VariantEntry, variantIndex: number): void {
        console.log(`[InstancedMultiMesh] grow → capacity=${entry.instanceBuffer.maxInstances}`);
        entry.instanceData.replaceTextures(entry.instanceBuffer.textures);
        for (let pi = 0; pi < entry.subMeshes.length; pi++) {
            const mesh = entry.subMeshes[pi];
            const mat = own(
                this.createMaterial(cloneStandardMaterial(entry.parts[pi].baseMaterial), entry.instanceData)
            );
            mesh.replaceMaterial(mat);
        }
        for (const attachment of this.attachments.values()) attachment.instanceDataChanged(this, variantIndex);
    }

    protected setInstance(key: number, variantIndex: number, bufIndex: number, values: Float32Array): boolean {
        const entry = this.variants[variantIndex];
        if (!entry) return false;

        const prevVariant = this.keyVariant.get(key);
        if (prevVariant !== undefined && prevVariant !== variantIndex) {
            this.variants[prevVariant]?.instanceBuffer.remove(key);
        }

        const ok = entry.instanceBuffer.copyToBuffer(key, bufIndex, values);
        if (ok) this.keyVariant.set(key, variantIndex);
        return ok;
    }

    get geometry(): THREE.BufferGeometry {
        return this.sourceGeo;
    }

    removeInstance(key: number): boolean {
        const variantIndex = this.keyVariant.get(key);
        if (variantIndex === undefined) return false;
        this.keyVariant.delete(key);
        return this.variants[variantIndex]?.instanceBuffer.remove(key) ?? false;
    }

    removeAll(): void {
        this.keyVariant.clear();
        for (const entry of this.variants) entry.instanceBuffer.clear();
    }

    get variantCount(): number {
        return this.variants.length;
    }

    getVariantParts(variantIndex: number): readonly SubMeshDef[] | undefined {
        return this.variants[variantIndex]?.parts;
    }

    instanceCount(variantIndex: number): number {
        return this.variants[variantIndex]?.instanceBuffer.length ?? 0;
    }

    setVariantVisible(variantIndex: number, visible: boolean): void {
        const entry = this.variants[variantIndex];
        if (!entry) return;
        for (const mesh of entry.subMeshes) mesh.visible = visible;
    }

    instances(variantIndex: number): IterableIterator<number> {
        return this.variants[variantIndex]?.instanceBuffer.keys ?? [][Symbol.iterator]();
    }

    getInstanceData(variantIndex: number): InstanceData | undefined {
        return this.variants[variantIndex]?.instanceData;
    }

    // Attaches a named add-on, e.g. `tile.attach('normals', new InstancedNormalLineAttachment(...))`.
    // Calls attachment.attach(this) to do the actual wiring — the attachment's constructor takes no
    // target reference, so there's nothing to mismatch. Replacing an existing name disposes the old
    // one first. Returns `attachment` so the call site can keep a typed reference if it needs one.
    attach<T extends InstancedMeshAttachment>(name: string, attachment: T): T {
        this.attachments.get(name)?.dispose();
        attachment.attach(this);
        this.attachments.set(name, attachment);
        return attachment;
    }

    // Disposes and removes the named attachment, if any.
    detach(name: string): void {
        this.attachments.get(name)?.dispose();
        this.attachments.delete(name);
    }

    dispose(): void {
        for (const attachment of this.attachments.values()) attachment.dispose();
        this.attachments.clear();
        for (const entry of this.variants) {
            entry.instanceBuffer.dispose();
            for (const mesh of entry.subMeshes) mesh.dispose();
        }
        this.variants.length = 0;
        disposeIfOwned(this.sourceGeo);
        this.group.parent?.remove(this.group);
    }
}
