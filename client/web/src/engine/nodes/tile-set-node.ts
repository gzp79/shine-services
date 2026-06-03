import * as THREE from 'three';
import { attribute, instanceIndex, mix, storage, vec3 } from 'three/tsl';
import { MeshBasicNodeMaterial, StorageInstancedBufferAttribute } from 'three/webgpu';
import { MultiRangeInstancedNode, MultiRangeInstancedParams } from './multi-range-instanced-node';

/**
 * Per-tile trilinear distortion: 8 control points (vec3 each) = 24 floats.
 *
 * Corner UV mapping:
 *   cp[0]:(0,0,0)  cp[1]:(1,0,0)  cp[2]:(0,1,0)  cp[3]:(1,1,0)
 *   cp[4]:(0,0,1)  cp[5]:(1,0,1)  cp[6]:(0,1,1)  cp[7]:(1,1,1)
 */
export type TileDistortion = Float32Array; // 24 floats

const CP_COUNT = 8;

export class TileSetNode extends MultiRangeInstancedNode {
    constructor(parent: THREE.Object3D, params: MultiRangeInstancedParams) {
        super(parent, params);
        this.init();
    }

    protected floatsPerInstance(): number {
        return CP_COUNT * 3; // 8 vec3 control points
    }

    protected createMaterial(_rangeIndex: number, storageAttr: StorageInstancedBufferAttribute): MeshBasicNodeMaterial {
        const cpBuffer = storage(storageAttr, 'vec3', storageAttr.count).toReadOnly();
        const base = instanceIndex.mul(CP_COUNT);
        const cp = (i: number) => cpBuffer.element(base.add(i));

        const p = attribute('position', 'vec3');
        const c00 = mix(cp(0), cp(1), p.x);
        const c01 = mix(cp(2), cp(3), p.x);
        const c10 = mix(cp(4), cp(5), p.x);
        const c11 = mix(cp(6), cp(7), p.x);
        const c0 = mix(c00, c01, p.y);
        const c1 = mix(c10, c11, p.y);

        const mat = new MeshBasicNodeMaterial({ side: THREE.DoubleSide });
        mat.positionNode = vec3(mix(c0, c1, p.z));
        return mat;
    }

    setTile(rangeIndex: number, key: number, distortion: TileDistortion): boolean {
        return super.setInstance(rangeIndex, key, distortion);
    }

    removeTile(rangeIndex: number, key: number): boolean {
        return super.removeInstance(rangeIndex, key);
    }
}
