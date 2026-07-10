import * as THREE from 'three';
import { float, mat4, mix, positionLocal, vec3, vec4 } from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import { loadGltf } from '../render/asset-loader';
import { share } from '../render/ownership';
import {
    type InstanceBufferLayout,
    InstanceData,
    InstancedMultiMesh,
    type InstancedMultiMeshParams,
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

export class InstancedTileSet extends InstancedMultiMesh {
    private readonly _scratch = new Float32Array(40);

    constructor(parent: THREE.Object3D, params: InstancedMultiMeshParams) {
        super(parent, params);
    }

    static async fromGltf(
        parent: THREE.Object3D,
        url: string,
        params?: Omit<InstancedMultiMeshParams, 'geometry' | 'variants'>
    ): Promise<InstancedTileSet> {
        const asset = await loadGltf(url);
        const variants: VariantDef[] = asset.meshes.map((m) => ({
            parts: m.submeshes.map((s) => ({
                baseMaterial: share(s.material),
                indexStart: s.indexStart,
                indexEnd: s.indexEnd
            }))
        }));
        return new InstancedTileSet(parent, { geometry: share(asset.geometry), variants, ...params });
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
        this._scratch.set(matrix.elements, 0);
        this._scratch.set(distortion, 16);
        return super.setInstance(variantIndex, key, 0, this._scratch);
    }

    removeTile(variantIndex: number, key: number): boolean {
        return super.removeInstance(variantIndex, key);
    }
}
