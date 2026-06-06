import * as THREE from 'three';
import { instanceIndex, ivec2, mix, positionLocal, textureLoad, vec3 } from 'three/tsl';
import { MeshBasicNodeMaterial } from 'three/webgpu';
import {
    type InstanceBufferLayout,
    MultiRangeInstancedNode,
    MultiRangeInstancedParams
} from './multi-range-instanced-node';

/**
 * Per-tile trilinear distortion: 8 control points (vec3 each) = 24 floats = 6 RGBA texels.
 *
 * Corner UV mapping:
 *   cp[0]:(0,0,0)  cp[1]:(1,0,0)  cp[2]:(0,1,0)  cp[3]:(1,1,0)
 *   cp[4]:(0,0,1)  cp[5]:(1,0,1)  cp[6]:(0,1,1)  cp[7]:(1,1,1)
 *
 * Packing: 8 vec3 packed as 6 vec4 texels (last float of each even texel and last float of last texel unused):
 *   texel 0: [cp0.x, cp0.y, cp0.z, cp1.x]
 *   texel 1: [cp1.y, cp1.z, cp2.x, cp2.y]
 *   texel 2: [cp2.z, cp3.x, cp3.y, cp3.z]
 *   texel 3: [cp4.x, cp4.y, cp4.z, cp5.x]
 *   texel 4: [cp5.y, cp5.z, cp6.x, cp6.y]
 *   texel 5: [cp6.z, cp7.x, cp7.y, cp7.z]
 */
export type TileDistortion = Float32Array; // 24 floats

const CP_COUNT = 8;

export class TileSetNode extends MultiRangeInstancedNode {
    constructor(parent: THREE.Object3D, params: MultiRangeInstancedParams) {
        super(parent, params);
        this.init();
    }

    protected instanceBufferLayout(): InstanceBufferLayout {
        return { buffers: [{ floatsPerInstance: CP_COUNT * 3 }] };
    }

    protected createMaterial(_rangeIndex: number, textures: readonly THREE.DataTexture[]): MeshBasicNodeMaterial {
        const tex = textures[0];
        // Texture layout: width=maxInstances (x=instanceIndex), height=texelsPerInstance (y=texelOffset)
        const iIdx = instanceIndex.toInt();
        const t = (row: number) => textureLoad(tex, ivec2(row, iIdx));
        const t0 = t(0);
        const t1 = t(1);
        const t2 = t(2);
        const t3 = t(3);
        const t4 = t(4);
        const t5 = t(5);

        const cp = [
            vec3(t0.x, t0.y, t0.z), // cp0
            vec3(t0.w, t1.x, t1.y), // cp1
            vec3(t1.z, t1.w, t2.x), // cp2
            vec3(t2.y, t2.z, t2.w), // cp3
            vec3(t3.x, t3.y, t3.z), // cp4
            vec3(t3.w, t4.x, t4.y), // cp5
            vec3(t4.z, t4.w, t5.x), // cp6
            vec3(t5.y, t5.z, t5.w) // cp7
        ];

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

    setTile(rangeIndex: number, key: number, distortion: TileDistortion): boolean {
        return super.setInstanceBuffer(rangeIndex, key, 0, distortion);
    }

    removeTile(rangeIndex: number, key: number): boolean {
        return super.removeInstance(rangeIndex, key);
    }
}
