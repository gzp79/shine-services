import * as THREE from 'three';
import { float, mat4, positionLocal, vec3, vec4 } from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import {
    type InstanceBufferLayout,
    InstanceData,
    InstancedMultiMesh,
    type InstancedMultiMeshParams
} from './instanced-multi-mesh';

export type { VariantDef, SubMeshDef, InstancedMultiMeshParams } from './instanced-multi-mesh';

// Buffer layout (single buffer, 20 floats = 5 texels):
//   texels 0-3: mat4 transform, column-major (col0..col3 each as vec4)
//   texel  4:   vec4 color (RGBA)

export class InstancedColorMesh extends InstancedMultiMesh {
    constructor(parent: THREE.Object3D, params: InstancedMultiMeshParams) {
        super(parent, params);
    }

    protected instanceBufferLayout(): InstanceBufferLayout {
        return { buffers: [{ floatsPerInstance: 20 }] };
    }

    protected createMaterial(_materialName: string, instanceData: InstanceData): MeshStandardNodeMaterial {
        const col0 = instanceData.vec4(0, 0);
        const col1 = instanceData.vec4(0, 1);
        const col2 = instanceData.vec4(0, 2);
        const col3 = instanceData.vec4(0, 3);
        const color = instanceData.vec4(0, 4);

        const instanceMatrix = mat4(col0, col1, col2, col3);
        const localPos = vec4(positionLocal, float(1.0));
        const transformed = instanceMatrix.mul(localPos);

        const mat = new MeshStandardNodeMaterial({ roughness: 0.6, metalness: 0.2 });
        mat.positionNode = vec3(transformed.x, transformed.y, transformed.z);
        mat.colorNode = vec3(color.x, color.y, color.z);
        return mat;
    }

    setObject(variantIndex: number, key: number, matrix: THREE.Matrix4, color: THREE.Color): boolean {
        const data = new Float32Array(20);
        data.set(matrix.elements, 0); // 16 floats, column-major
        data[16] = color.r;
        data[17] = color.g;
        data[18] = color.b;
        data[19] = 1.0;
        return this.setInstance(variantIndex, key, 0, data);
    }

    removeObject(variantIndex: number, key: number): boolean {
        return this.removeInstance(variantIndex, key);
    }
}
