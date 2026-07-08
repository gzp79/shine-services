import * as THREE from 'three';
import { mix, positionLocal, vec3 } from 'three/tsl';
import { MeshBasicNodeMaterial } from 'three/webgpu';
import {
    type InstanceBufferLayout,
    InstanceData,
    InstancedMultiMesh,
    type InstancedMultiMeshParams
} from './instanced-multi-mesh';

export type { SubMeshDef, TileTypeDef, InstancedMultiMeshParams } from './instanced-multi-mesh';

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
    constructor(parent: THREE.Object3D, params: InstancedMultiMeshParams) {
        super(parent, params);
    }

    protected instanceBufferLayout(): InstanceBufferLayout {
        return { buffers: [{ floatsPerInstance: CP_COUNT * 3 }] };
    }

    protected createMaterial(_materialName: string, instanceData: InstanceData): MeshBasicNodeMaterial {
        const cp = Array.from({ length: CP_COUNT }, (_, i) => instanceData.vec3(0, i));

        const p = positionLocal;
        const c00 = mix(cp[0], cp[1], p.x);
        const c01 = mix(cp[2], cp[3], p.x);
        const c10 = mix(cp[4], cp[5], p.x);
        const c11 = mix(cp[6], cp[7], p.x);
        const c0 = mix(c00, c01, p.y);
        const c1 = mix(c10, c11, p.y);

        const mat = new MeshBasicNodeMaterial({ side: THREE.DoubleSide });
        mat.positionNode = vec3(mix(c0, c1, p.z));
        return mat;
    }

    setTile(tileTypeIndex: number, key: number, distortion: TileDistortion): boolean {
        return super.setInstance(tileTypeIndex, key, 0, distortion);
    }

    removeTile(tileTypeIndex: number, key: number): boolean {
        return super.removeInstance(tileTypeIndex, key);
    }
}
