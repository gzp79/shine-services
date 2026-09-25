import * as THREE from 'three';
import {
    Fn,
    float,
    mat4,
    normalLocal,
    normalize,
    positionLocal,
    transformNormalToView,
    varyingProperty,
    vec3,
    vec4
} from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import {
    type InstanceBufferLayout,
    InstanceData,
    InstancedMultiMesh,
    type InstancedMultiMeshParams
} from './instanced-multi-mesh';

export type { VariantDef, SubMeshDef, InstancedMultiMeshParams } from './instanced-multi-mesh';

/** Buffer layout (single buffer, 20 floats = 5 texels):
 *   floats  0-15: mat4 instance transform, column-major
 *   floats 16-19: vec4 color (RGBA)
 */
export const COLOR_INSTANCE_SCHEMA = Symbol('InstancedColorMesh.instanceData');
const NORMAL_VARYING = 'vColorNormal';

export class InstancedColorMesh extends InstancedMultiMesh {
    private readonly _scratch = new Float32Array(20);

    constructor(parent: THREE.Object3D, params: InstancedMultiMeshParams) {
        super(parent, params);
    }

    protected instanceBufferLayout(): InstanceBufferLayout {
        return { schema: COLOR_INSTANCE_SCHEMA, buffers: [{ floatsPerInstance: 20 }] };
    }

    // Position and unit normal after the rigid instance transform.
    static computeWarp(instanceData: InstanceData) {
        if (instanceData.schema !== COLOR_INSTANCE_SCHEMA) {
            throw new Error('InstancedColorMesh.computeWarp: instanceData is not from an InstancedColorMesh');
        }

        const col0 = instanceData.vec4(0, 0);
        const col1 = instanceData.vec4(0, 1);
        const col2 = instanceData.vec4(0, 2);
        const col3 = instanceData.vec4(0, 3);
        const instanceMatrix = mat4(col0, col1, col2, col3);

        const transformed = instanceMatrix.mul(vec4(positionLocal, float(1.0)));
        const position = vec3(transformed.x, transformed.y, transformed.z);

        // Instance matrix is assumed rotation + uniform scale (no shear), so its linear part can be
        // applied directly rather than its inverse-transpose — same assumption InstancedTileSet makes
        // (and how three.js's own InstancedMesh handles normals).
        const normal = normalize(instanceMatrix.toMat3().mul(normalLocal));

        return { position, normal };
    }

    protected createMaterial(mat: MeshStandardNodeMaterial, instanceData: InstanceData): MeshStandardNodeMaterial {
        const color = instanceData.vec4(0, 4);

        const normalVarying = varyingProperty('vec3', NORMAL_VARYING);
        mat.positionNode = Fn(() => {
            const { position, normal } = InstancedColorMesh.computeWarp(instanceData);
            normalVarying.assign(normal);
            return position;
        })();
        mat.normalNode = transformNormalToView(normalVarying);
        mat.colorNode = vec3(color.x, color.y, color.z);
        return mat;
    }

    setObject(key: number, variantIndex: number, matrix: THREE.Matrix4, color: THREE.Color): boolean {
        this._scratch.set(matrix.elements, 0);
        this._scratch[16] = color.r;
        this._scratch[17] = color.g;
        this._scratch[18] = color.b;
        this._scratch[19] = 1.0;
        return this.setInstance(key, variantIndex, 0, this._scratch);
    }

    removeObject(key: number): boolean {
        return this.removeInstance(key);
    }
}
