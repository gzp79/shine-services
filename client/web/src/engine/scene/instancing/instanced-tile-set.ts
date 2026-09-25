import * as THREE from 'three';
import {
    Fn,
    cross,
    float,
    mat4,
    mix,
    normalLocal,
    normalize,
    positionLocal,
    transformNormalToView,
    varyingProperty,
    vec3,
    vec4
} from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import { type ModelSet, toModelSet } from '../../assets/model-set';
import { loadGltf } from '../../loaders/gltf-loader';
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

/** Buffer layout (single buffer, 40 floats = 10 texels):
 *   floats  0-15: mat4 instance transform, column-major
 *   floats 16-39: cp[0..7] as 8×vec3, the TileDistortion control points
 */
export const TILE_INSTANCE_SCHEMA = Symbol('InstancedTileSet.instanceData');

const CP_COUNT = 8;
const NORMAL_VARYING = 'vTileNormal';

function toVariants(modelSet: ModelSet): VariantDef[] {
    return modelSet.models.map((m) => ({
        parts: m.parts.map((p) => ({
            baseMaterial: p.material,
            indexStart: p.indexStart,
            indexEnd: p.indexEnd
        }))
    }));
}

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
        const modelSet = toModelSet(await loadGltf(url), 'owned');
        return new InstancedTileSet(parent, {
            geometry: modelSet.geometry,
            variants: toVariants(modelSet),
            ...params
        });
    }

    // Builds from a store-loaded ModelSet. Its geometry and materials are already shared —
    // the store owns them, so this tile set must not dispose them.
    static fromModelSet(
        parent: THREE.Object3D,
        modelSet: ModelSet,
        params?: Omit<InstancedMultiMeshParams, 'geometry' | 'variants'>
    ): InstancedTileSet {
        return new InstancedTileSet(parent, {
            geometry: modelSet.geometry,
            variants: toVariants(modelSet),
            ...params
        });
    }

    protected instanceBufferLayout(): InstanceBufferLayout {
        return { schema: TILE_INSTANCE_SCHEMA, buffers: [{ floatsPerInstance: 40 }] };
    }

    // Position and unit normal after the trilinear warp + instance transform, both in the same
    // pre-modelMatrix space as positionLocal. Static and public: it's a pure function of
    // `instanceData` (no instance state), so it can be passed by reference — e.g. as the NormalWarp
    // an InstancedNormalLineAttachment reuses to draw the exact same math as createMaterial.
    static computeWarp(instanceData: InstanceData) {
        if (instanceData.schema !== TILE_INSTANCE_SCHEMA) {
            throw new Error('InstancedTileSet.computeWarp: instanceData is not from an InstancedTileSet');
        }

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
        const warpedPosition = mix(c0, c1, p.z);
        const instancePosition = instanceMatrix.mul(vec4(warpedPosition, float(1.0)));
        const position = vec3(instancePosition.x, instancePosition.y, instancePosition.z);

        const n = normalLocal;
        // Jacobian of the trilinear warp (columns d(distorted)/dx,dy,dz), from the same mixes above.
        const dDdx = mix(
            mix(cp[1].sub(cp[0]), cp[3].sub(cp[2]), p.y),
            mix(cp[5].sub(cp[4]), cp[7].sub(cp[6]), p.y),
            p.z
        );
        const dDdy = mix(c01.sub(c00), c11.sub(c10), p.z);
        const dDdz = c1.sub(c0);
        // Inverse-transpose of the Jacobian applied to the normal, via the cofactor identity
        // (columns b×c, c×a, a×b) — avoids an explicit 3x3 inverse; normalize() below absorbs the determinant.
        const warpedNormal = cross(dDdy, dDdz).mul(n.x).add(cross(dDdz, dDdx).mul(n.y)).add(cross(dDdx, dDdy).mul(n.z));
        const instanceNormal = instanceMatrix.toMat3().mul(warpedNormal);
        const normal = normalize(instanceNormal);

        return { position, normal };
    }

    protected createMaterial(mat: MeshStandardNodeMaterial, instanceData: InstanceData): MeshStandardNodeMaterial {
        const normalVarying = varyingProperty('vec3', NORMAL_VARYING);
        mat.positionNode = Fn(() => {
            const { position, normal } = InstancedTileSet.computeWarp(instanceData);
            normalVarying.assign(normal);
            return position;
        })();
        mat.normalNode = transformNormalToView(normalVarying);
        return mat;
    }

    setTile(key: number, variantIndex: number, matrix: THREE.Matrix4, distortion: TileDistortion): boolean {
        this._scratch.set(matrix.elements, 0);
        this._scratch.set(distortion, 16);
        return super.setInstance(key, variantIndex, 0, this._scratch);
    }

    removeTile(key: number): boolean {
        return super.removeInstance(key);
    }
}
