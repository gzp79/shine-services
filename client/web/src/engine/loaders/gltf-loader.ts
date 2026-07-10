import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { MeshStandardNodeMaterial } from 'three/webgpu';

type GltfAccessor = {
    bufferView: number;
    byteOffset?: number;
    count: number;
    componentType: number;
    type: string;
    normalized?: boolean;
};
type GltfBufferView = { byteOffset?: number; byteLength: number; byteStride?: number };
type GltfPrimitive = { indices: number; material: number; attributes: Record<string, number> };
type GltfMesh = { name?: string; primitives: GltfPrimitive[] };
type GltfNode = { name?: string; mesh?: number };
type GltfScene = { nodes?: number[] };
type GltfJson = {
    scene?: number;
    scenes: GltfScene[];
    nodes: GltfNode[];
    meshes: GltfMesh[];
    accessors: GltfAccessor[];
    bufferViews: GltfBufferView[];
};

export type GltfSubMesh = {
    material: MeshStandardNodeMaterial;
    indexStart: number;
    indexEnd: number;
};

export type GltfMeshEntry = {
    name: string;
    submeshes: GltfSubMesh[];
};

export type GltfAsset = {
    geometry: THREE.BufferGeometry;
    meshes: GltfMeshEntry[];
};

const ATTRIB_NAME: Record<string, string> = {
    POSITION: 'position',
    NORMAL: 'normal',
    TEXCOORD_0: 'uv',
    COLOR_0: 'color'
};

const ITEM_SIZES: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 };

type AttribDesc = { byteOffset: number; byteStride: number; itemSize: number; normalized: boolean };

function collectAttribs(json: GltfJson, prims: GltfPrimitive[]): Map<string, AttribDesc> {
    const attribs = new Map<string, AttribDesc>();
    for (const prim of prims) {
        for (const [name, accIdx] of Object.entries(prim.attributes)) {
            const acc = json.accessors[accIdx];
            const bv = json.bufferViews[acc.bufferView];
            const itemSize = ITEM_SIZES[acc.type] ?? 1;
            const byteStride = bv.byteStride ?? itemSize * 4;
            const desc: AttribDesc = {
                byteOffset: acc.byteOffset ?? 0,
                byteStride,
                itemSize,
                normalized: acc.normalized ?? false
            };
            const existing = attribs.get(name);
            if (!existing) {
                attribs.set(name, desc);
            } else if (existing.byteStride !== desc.byteStride || existing.itemSize !== desc.itemSize) {
                throw new Error(
                    `[loadGltf] attribute "${name}" is inconsistent across primitives: ` +
                        `stride ${existing.byteStride} vs ${desc.byteStride}, ` +
                        `itemSize ${existing.itemSize} vs ${desc.itemSize}`
                );
            }
        }
    }
    return attribs;
}

function buildGeometry(
    json: GltfJson,
    rawBuffer: ArrayBuffer,
    prims: GltfPrimitive[],
    attribs: Map<string, AttribDesc>,
    vertexCounts: number[]
): THREE.BufferGeometry {
    const totalVertices = vertexCounts.reduce((a, b) => a + b, 0);

    const merged = new Map<string, Float32Array>();
    for (const [name, desc] of attribs) {
        merged.set(name, new Float32Array(totalVertices * desc.itemSize));
    }

    let vertexOffset = 0;
    for (let pi = 0; pi < prims.length; pi++) {
        const prim = prims[pi];
        const count = vertexCounts[pi];
        for (const [name, desc] of attribs) {
            const accIdx = prim.attributes[name];
            if (accIdx === undefined) continue;
            const acc = json.accessors[accIdx];
            const bv = json.bufferViews[acc.bufferView];
            const bvOff = (bv.byteOffset ?? 0) + (acc.byteOffset ?? 0);
            const stride = desc.byteStride / 4;
            const srcLength = (count - 1) * stride + desc.itemSize;
            const dst = merged.get(name)!;
            const src = new Float32Array(rawBuffer, bvOff, srcLength);
            for (let v = 0; v < count; v++) {
                for (let c = 0; c < desc.itemSize; c++) {
                    dst[(vertexOffset + v) * desc.itemSize + c] = src[v * stride + c];
                }
            }
        }
        vertexOffset += count;
    }

    const geometry = new THREE.BufferGeometry();
    for (const [gltfName, desc] of attribs) {
        geometry.setAttribute(
            ATTRIB_NAME[gltfName] ?? gltfName.toLowerCase(),
            new THREE.BufferAttribute(merged.get(gltfName)!, desc.itemSize, desc.normalized)
        );
    }
    return geometry;
}

function buildIndexBuffer(
    json: GltfJson,
    rawBuffer: ArrayBuffer,
    prims: GltfPrimitive[],
    vertexCounts: number[]
): { attr: THREE.BufferAttribute; ranges: { indexStart: number; indexEnd: number }[] } {
    const indexCounts = prims.map((p) => json.accessors[p.indices].count);
    const totalIndices = indexCounts.reduce((a, b) => a + b, 0);
    const totalVertices = vertexCounts.reduce((a, b) => a + b, 0);
    const merged = totalVertices > 65535 ? new Uint32Array(totalIndices) : new Uint16Array(totalIndices);

    const ranges: { indexStart: number; indexEnd: number }[] = [];
    let indexOffset = 0;
    let vertexOffset = 0;
    for (let pi = 0; pi < prims.length; pi++) {
        const acc = json.accessors[prims[pi].indices];
        const bv = json.bufferViews[acc.bufferView];
        const bvOff = (bv.byteOffset ?? 0) + (acc.byteOffset ?? 0);
        const count = indexCounts[pi];
        const isU32 = acc.componentType === 5125;
        const src = isU32 ? new Uint32Array(rawBuffer, bvOff, count) : new Uint16Array(rawBuffer, bvOff, count);
        for (let i = 0; i < count; i++) merged[indexOffset + i] = src[i] + vertexOffset;
        ranges.push({ indexStart: indexOffset, indexEnd: indexOffset + count });
        indexOffset += count;
        vertexOffset += vertexCounts[pi];
    }

    return { attr: new THREE.BufferAttribute(merged, 1), ranges };
}

async function toNodeMaterial(
    parser: { getDependency(type: string, idx: number): Promise<unknown> },
    matIdx: number
): Promise<MeshStandardNodeMaterial> {
    const classic = (await parser.getDependency('material', matIdx)) as THREE.MeshStandardMaterial;
    const mat = new MeshStandardNodeMaterial();
    THREE.MeshStandardMaterial.prototype.copy.call(mat, classic);
    return mat;
}

function disposeGltfScene(scene: THREE.Object3D): void {
    scene.traverse((obj) => {
        if (obj instanceof THREE.Mesh) {
            obj.geometry.dispose();
            if (Array.isArray(obj.material)) obj.material.forEach((m) => m.dispose());
            else obj.material.dispose();
        }
    });
}

export async function loadGltf(url: string): Promise<GltfAsset> {
    const loader = new GLTFLoader();
    const gltf = await loader.loadAsync(url);
    const json = gltf.parser.json as GltfJson;
    const sceneJson = json.scenes[json.scene ?? 0];
    const rawBuffer = (await gltf.parser.loadBuffer(0)) as ArrayBuffer;

    const rootNodeSet = new Set(sceneJson.nodes ?? []);
    const skippedCount = json.nodes.filter((n, i) => n.mesh !== undefined && !rootNodeSet.has(i)).length;
    if (skippedCount > 0) {
        console.warn(`[loadGltf] ${skippedCount} mesh node(s) skipped — not scene-root nodes`);
    }

    const meshNodeIndices = (sceneJson.nodes ?? []).filter((ni) => json.nodes[ni].mesh !== undefined);
    if (meshNodeIndices.length === 0) throw new Error('loadGltf: no mesh nodes found in scene');

    const allPrims = meshNodeIndices.flatMap((ni) => json.meshes[json.nodes[ni].mesh!].primitives);

    const attribs = collectAttribs(json, allPrims);
    const vertexCounts = allPrims.map((p) => json.accessors[p.attributes.POSITION].count);
    const geometry = buildGeometry(json, rawBuffer, allPrims, attribs, vertexCounts);
    const { attr: indexAttr, ranges } = buildIndexBuffer(json, rawBuffer, allPrims, vertexCounts);
    geometry.setIndex(indexAttr);

    const meshes: GltfMeshEntry[] = [];
    let globalPrimIdx = 0;
    for (const ni of meshNodeIndices) {
        const node = json.nodes[ni];
        const gltfMesh = json.meshes[node.mesh!];
        const name = node.name ?? gltfMesh.name ?? `node_${ni}`;
        const primRanges = gltfMesh.primitives.map((_, pi) => ranges[globalPrimIdx + pi]);
        globalPrimIdx += gltfMesh.primitives.length;
        const submeshes = await Promise.all(
            gltfMesh.primitives.map(async (prim, pi) => {
                if (prim.material === undefined) {
                    console.warn(`[loadGltf] primitive in "${name}" has no material, using default`);
                    const { indexStart, indexEnd } = primRanges[pi];
                    return { material: new MeshStandardNodeMaterial(), indexStart, indexEnd };
                }
                const { indexStart, indexEnd } = primRanges[pi];
                const material = await toNodeMaterial(gltf.parser, prim.material);
                return { material, indexStart, indexEnd };
            })
        );
        meshes.push({ name, submeshes });
    }

    disposeGltfScene(gltf.scene);

    console.log(`[loadGltf] loaded ${meshes.length} mesh(es), ${allPrims.length} primitives.`);
    return { geometry, meshes };
}
