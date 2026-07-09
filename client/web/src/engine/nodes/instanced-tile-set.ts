import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { float, mat4, mix, positionLocal, vec3, vec4 } from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import {
    type InstanceBufferLayout,
    InstanceData,
    InstancedMultiMesh,
    type InstancedMultiMeshParams,
    type SubMeshDef,
    type VariantDef
} from './instanced-multi-mesh';

export type { SubMeshDef, VariantDef, InstancedMultiMeshParams } from './instanced-multi-mesh';

/**
 * Per-tile trilinear distortion: 8 control points (vec3 each).
 *
 * Corner mapping:
 *   cp[0]:(0,0,0)  cp[1]:(1,0,0)  cp[2]:(0,1,0)  cp[3]:(1,1,0)
 *   cp[4]:(0,0,1)  cp[5]:(1,0,1)  cp[6]:(0,1,1)  cp[7]:(1,1,1)
 */
export type TileDistortion = Float32Array; // 24 floats

const CP_COUNT = 8;

type GltfAccessor = { bufferView: number; byteOffset?: number; count: number; componentType: number };
type GltfBufferView = { byteOffset?: number };
type GltfPrimitive = { indices: number; material: number };
type GltfMesh = { primitives: GltfPrimitive[] };
type GltfNode = { mesh?: number };
type GltfScene = { nodes?: number[] };
type GltfJson = {
    scene?: number;
    scenes: GltfScene[];
    nodes: GltfNode[];
    meshes: GltfMesh[];
    accessors: GltfAccessor[];
    bufferViews: GltfBufferView[];
};

export class InstancedTileSet extends InstancedMultiMesh {
    constructor(parent: THREE.Object3D, params: InstancedMultiMeshParams) {
        super(parent, params);
    }

    static async fromGltf(
        parent: THREE.Object3D,
        url: string,
        params?: Omit<InstancedMultiMeshParams, 'geometry' | 'variants'>
    ): Promise<InstancedTileSet> {
        const loader = new GLTFLoader();
        const gltf = await loader.loadAsync(url);

        // Extract the single shared geometry from the first mesh in the scene
        const firstMesh = gltf.scene.getObjectByProperty('isMesh', true) as THREE.Mesh;
        if (!firstMesh) throw new Error('fromGltf: no mesh found in glTF scene');
        const geometry = firstMesh.geometry;

        const json = gltf.parser.json as GltfJson;
        const sceneJson = json.scenes[json.scene ?? 0];
        const variants: VariantDef[] = [];

        for (const nodeIdx of sceneJson.nodes ?? []) {
            const nodeJson = json.nodes[nodeIdx];
            if (nodeJson.mesh === undefined) continue;

            const meshPrims = json.meshes[nodeJson.mesh].primitives;
            const parts: SubMeshDef[] = [];

            for (const prim of meshPrims) {
                const acc = json.accessors[prim.indices];
                const bv = json.bufferViews[acc.bufferView];
                // UNSIGNED_INT=5125 (4 bytes), UNSIGNED_SHORT=5123 (2 bytes)
                const componentSize = acc.componentType === 5125 ? 4 : 2;
                const byteOffset = (bv.byteOffset ?? 0) + (acc.byteOffset ?? 0);
                const indexStart = byteOffset / componentSize;
                const indexEnd = indexStart + acc.count;
                const baseMaterial = (await gltf.parser.getDependency(
                    'material',
                    prim.material
                )) as MeshStandardNodeMaterial;
                parts.push({ baseMaterial, indexStart, indexEnd });
            }

            variants.push({ parts });
        }

        return new InstancedTileSet(parent, { geometry, variants, ...params });
    }

    // Instance data: 40 floats = 10 texels
    //   floats  0-15: mat4 transform, column-major
    //   floats 16-39: cp[0..7] as 8×vec3
    protected instanceBufferLayout(): InstanceBufferLayout {
        return { buffers: [{ floatsPerInstance: 40 }] };
    }

    protected createMaterial(mat: MeshStandardNodeMaterial, instanceData: InstanceData): MeshStandardNodeMaterial {
        const col0 = instanceData.vec4(0, 0);
        const col1 = instanceData.vec4(0, 1);
        const col2 = instanceData.vec4(0, 2);
        const col3 = instanceData.vec4(0, 3);
        const cp = Array.from({ length: CP_COUNT }, (_, i) => instanceData.vec3At(0, 16 + i * 3));

        const instanceMatrix = mat4(col0, col1, col2, col3);
        const p = positionLocal;
        const c00 = mix(cp[0], cp[1], p.x);
        const c01 = mix(cp[2], cp[3], p.x);
        const c10 = mix(cp[4], cp[5], p.x);
        const c11 = mix(cp[6], cp[7], p.x);
        const c0 = mix(c00, c01, p.y);
        const c1 = mix(c10, c11, p.y);
        const distorted = mix(c0, c1, p.z);

        const localPos = vec4(distorted, float(1.0));
        const transformed = instanceMatrix.mul(localPos);

        mat.positionNode = vec3(transformed.x, transformed.y, transformed.z);
        return mat;
    }

    setTile(variantIndex: number, key: number, matrix: THREE.Matrix4, distortion: TileDistortion): boolean {
        const data = new Float32Array(40);
        data.set(matrix.elements, 0);
        data.set(distortion, 16);
        return super.setInstance(variantIndex, key, 0, data);
    }

    removeTile(variantIndex: number, key: number): boolean {
        return super.removeInstance(variantIndex, key);
    }
}
