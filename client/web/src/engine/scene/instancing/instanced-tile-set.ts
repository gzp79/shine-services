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
import { MeshStandardNodeMaterial, type Node } from 'three/webgpu';
import type { TileDistortionLike } from '../../../mesh/polygon-mesh';
import type { ModelSet } from '../../assets/model-set';
import {
    type InstanceBufferLayout,
    InstanceData,
    InstancedMultiMesh,
    type InstancedMultiMeshParams,
    type VariantDef
} from './instanced-multi-mesh';
import { TileInstanceResolver } from './tile-instance-resolver';
import {
    warpCoonsBell,
    warpPiecewiseBilinear,
    warpPiecewiseBilinearBlended,
    warpPiecewiseBilinearCoonsNormal,
    warpPiecewiseBilinearCornerNormal,
    warpPiecewiseBilinearSecantNormal,
    warpPiecewiseBilinearSmoothNormal,
    warpQ9,
    warpRationalBezier
} from './tile-warp-q9';

export type { SubMeshDef, VariantDef, InstancedMultiMeshParams } from './instanced-multi-mesh';

export const TILE_VARIANT_NAMES = ['q0000', 'q1000', 'q1100', 'q1001', 'q1110', 'q1111'] as const;
export type TileVariantName = (typeof TILE_VARIANT_NAMES)[number];

export const TILE_DISTORTION_METHODS = [
    'trilinear',
    'lagrangeQ9',
    'coonsBell',
    'rationalBezier',
    'piecewiseBilinear',
    'piecewiseBilinearBlended',
    'piecewiseBilinearSmoothNormal',
    'piecewiseBilinearCoonsNormal',
    'piecewiseBilinearCornerNormal',
    'piecewiseBilinearSecantNormal'
] as const;
export type TileDistortionMethod = (typeof TILE_DISTORTION_METHODS)[number];

export type InstancedTileSetParams = InstancedMultiMeshParams & {
    method: TileDistortionMethod;
    height: number;
};

/** Trilinear tile distortion control points buffer layout.
 *   single buffer, 40 floats = 10 texels
 *   floats  0-15: mat4 instance transform, column-major
 *   floats 16-39: cp[0..7] as 8 × vec3, the control points where
 *     cp[0]:(0,0,0)  cp[1]:(1,0,0)  cp[2]:(0,1,0)  cp[3]:(1,1,0)
 *     cp[4]:(0,0,1)  cp[5]:(1,0,1)  cp[6]:(0,1,1)  cp[7]:(1,1,1)
 */
export const TILE_TRILINEAR_SCHEMA = Symbol('InstancedTileSet.instanceData');

/** Q9 tile distortion control points buffer layout, shared by every corners+edge-blends based method
 *  ('lagrangeQ9', 'coonsBell', 'rationalBezier', 'piecewiseBilinear', 'piecewiseBilinearBlended',
 *  'piecewiseBilinearSmoothNormal', 'piecewiseBilinearCoonsNormal', 'piecewiseBilinearCornerNormal',
 *  'piecewiseBilinearSecantNormal', ...).
 *   single buffer, 32 floats = 8 texels
 *   floats  0-15: mat4 instance transform, column-major
 *   floats 16-23: cp[0..3] as 4 × vec2 corners, row-major BL, BR, TL, TR (same layout as trilinear's
 *     cp[0..3], but z-less: these methods warp xy only, height scales z separately)
 *   floats 24-27: per-edge blend factors [bottom(cp0-cp1), right(cp1-cp3), top(cp3-cp2), left(cp2-cp0)]
 *   float     28: height
 *   floats 29-31: unused padding
 */
export const TILE_Q9_SCHEMA = Symbol('InstancedTileSet.instanceDataQ9');

const NORMAL_VARYING = 'vTileNormal';

export class InstancedTileSet extends InstancedMultiMesh {
    private readonly method: TileDistortionMethod;
    private readonly height: number;
    private readonly _scratch = new Float32Array(40);
    private readonly resolver: TileInstanceResolver;

    private constructor(parent: THREE.Object3D, modelSet: ModelSet, params: InstancedTileSetParams) {
        super(
            parent,
            modelSet.geometry,
            toVariants(modelSet),
            InstancedTileSet.instanceBufferLayout(params.method),
            params
        );
        this.method = params.method;
        this.height = params.height;
        this.resolver = new TileInstanceResolver(modelSet);
    }

    static fromModelSet(parent: THREE.Object3D, modelSet: ModelSet, params: InstancedTileSetParams): InstancedTileSet {
        return new InstancedTileSet(parent, modelSet, params);
    }

    static instanceBufferLayout(method: TileDistortionMethod): InstanceBufferLayout {
        switch (method) {
            case 'trilinear':
                return { schema: TILE_TRILINEAR_SCHEMA, buffers: [{ floatsPerInstance: 40 }] };
            case 'lagrangeQ9':
            case 'coonsBell':
            case 'rationalBezier':
            case 'piecewiseBilinear':
            case 'piecewiseBilinearBlended':
            case 'piecewiseBilinearSmoothNormal':
            case 'piecewiseBilinearCoonsNormal':
            case 'piecewiseBilinearCornerNormal':
            case 'piecewiseBilinearSecantNormal':
                return { schema: TILE_Q9_SCHEMA, buffers: [{ floatsPerInstance: 32 }] };
            default:
                throw new Error(`InstancedTileSet: unsupported distortion method ${method}`);
        }
    }

    // Perform position and unit normal distortion using the trilinear warp and instance transform.
    static computeTrilinearWarp(instanceData: InstanceData) {
        if (instanceData.schema !== TILE_TRILINEAR_SCHEMA) {
            throw new Error('InstancedTileSet.computeTrilinearWarp: instanceData is not from an InstancedTileSet');
        }

        const col0 = instanceData.vec4(0, 0);
        const col1 = instanceData.vec4(0, 1);
        const col2 = instanceData.vec4(0, 2);
        const col3 = instanceData.vec4(0, 3);
        const cp = Array.from({ length: 8 }, (_, i) => instanceData.vec3At(0, 16 + i * 3));

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
        const dDdx = mix(
            mix(cp[1].sub(cp[0]), cp[3].sub(cp[2]), p.y),
            mix(cp[5].sub(cp[4]), cp[7].sub(cp[6]), p.y),
            p.z
        );
        const dDdy = mix(c01.sub(c00), c11.sub(c10), p.z);
        const dDdz = c1.sub(c0);
        const warpedNormal = cross(dDdy, dDdz).mul(n.x).add(cross(dDdz, dDdx).mul(n.y)).add(cross(dDdx, dDdy).mul(n.z));
        const instanceNormal = instanceMatrix.toMat3().mul(warpedNormal);
        const normal = normalize(instanceNormal);

        return { position, normal };
    }

    // Reads the TILE_Q9_SCHEMA buffer fields shared by every corners+edge-blends based method.
    private static readQ9Buffer(instanceData: InstanceData, methodName: string) {
        if (instanceData.schema !== TILE_Q9_SCHEMA) {
            throw new Error(`InstancedTileSet.${methodName}: instanceData is not from an InstancedTileSet`);
        }

        const col0 = instanceData.vec4(0, 0);
        const col1 = instanceData.vec4(0, 1);
        const col2 = instanceData.vec4(0, 2);
        const col3 = instanceData.vec4(0, 3);
        const corners = Array.from({ length: 4 }, (_, i) => instanceData.vec2At(0, 16 + i * 2));
        const edgeBlends = Array.from({ length: 4 }, (_, i) => instanceData.floatAt(0, 24 + i));
        const height = instanceData.floatAt(0, 28);

        return { instanceMatrix: mat4(col0, col1, col2, col3), corners, edgeBlends, height };
    }

    // Applies the instance transform to a local-space warp result, matching computeTrilinearWarp's tail.
    private static applyInstanceTransform(
        instanceMatrix: ReturnType<typeof mat4>,
        warpedPosition: Node<'vec3'>,
        warpedNormal: Node<'vec3'>
    ) {
        const instancePosition = instanceMatrix.mul(vec4(warpedPosition, float(1.0)));
        const position = vec3(instancePosition.x, instancePosition.y, instancePosition.z);
        const instanceNormal = instanceMatrix.toMat3().mul(warpedNormal);
        const normal = normalize(instanceNormal);
        return { position, normal };
    }

    // Perform position and unit normal distortion using the Q9 Lagrange warp and instance transform.
    static computeLagrangeQ9Warp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computeLagrangeQ9Warp'
        );
        const { position, normal } = warpQ9(corners, edgeBlends, height, positionLocal, normalLocal);
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the Coons-bell warp and instance transform.
    static computeCoonsBellWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computeCoonsBellWarp'
        );
        const { position, normal } = warpCoonsBell(corners, edgeBlends, height, positionLocal, normalLocal);
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the rational-Bezier warp and instance transform.
    static computeRationalBezierWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computeRationalBezierWarp'
        );
        const { position, normal } = warpRationalBezier(corners, edgeBlends, height, positionLocal, normalLocal);
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the piecewise-bilinear warp and instance
    // transform.
    static computePiecewiseBilinearWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computePiecewiseBilinearWarp'
        );
        const { position, normal } = warpPiecewiseBilinear(corners, edgeBlends, height, positionLocal, normalLocal);
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the piecewise-bilinear-with-smoothed-center warp
    // and instance transform.
    static computePiecewiseBilinearBlendedWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computePiecewiseBilinearBlendedWarp'
        );
        const { position, normal } = warpPiecewiseBilinearBlended(
            corners,
            edgeBlends,
            height,
            positionLocal,
            normalLocal
        );
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the piecewise-bilinear-with-smoothed-normal warp
    // and instance transform.
    static computePiecewiseBilinearSmoothNormalWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computePiecewiseBilinearSmoothNormalWarp'
        );
        const { position, normal } = warpPiecewiseBilinearSmoothNormal(
            corners,
            edgeBlends,
            height,
            positionLocal,
            normalLocal
        );
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the piecewise-bilinear-geometry/Coons-normal
    // hybrid warp and instance transform.
    static computePiecewiseBilinearCoonsNormalWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computePiecewiseBilinearCoonsNormalWarp'
        );
        const { position, normal } = warpPiecewiseBilinearCoonsNormal(
            corners,
            edgeBlends,
            height,
            positionLocal,
            normalLocal
        );
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the piecewise-bilinear-geometry/corners-only
    // normal hybrid warp and instance transform.
    static computePiecewiseBilinearCornerNormalWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computePiecewiseBilinearCornerNormalWarp'
        );
        const { position, normal } = warpPiecewiseBilinearCornerNormal(
            corners,
            edgeBlends,
            height,
            positionLocal,
            normalLocal
        );
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Perform position and unit normal distortion using the piecewise-bilinear-geometry/secant-normal
    // hybrid warp and instance transform.
    static computePiecewiseBilinearSecantNormalWarp(instanceData: InstanceData) {
        const { instanceMatrix, corners, edgeBlends, height } = InstancedTileSet.readQ9Buffer(
            instanceData,
            'computePiecewiseBilinearSecantNormalWarp'
        );
        const { position, normal } = warpPiecewiseBilinearSecantNormal(
            corners,
            edgeBlends,
            height,
            positionLocal,
            normalLocal
        );
        return InstancedTileSet.applyInstanceTransform(instanceMatrix, position, normal);
    }

    // Selects the local-space warp for this tile set's method. Public so add-ons (e.g.
    // InstancedNormalLineAttachment) can reuse it without switching on InstanceData.schema themselves.
    computeWarp(instanceData: InstanceData) {
        switch (this.method) {
            case 'trilinear':
                return InstancedTileSet.computeTrilinearWarp(instanceData);
            case 'lagrangeQ9':
                return InstancedTileSet.computeLagrangeQ9Warp(instanceData);
            case 'coonsBell':
                return InstancedTileSet.computeCoonsBellWarp(instanceData);
            case 'rationalBezier':
                return InstancedTileSet.computeRationalBezierWarp(instanceData);
            case 'piecewiseBilinear':
                return InstancedTileSet.computePiecewiseBilinearWarp(instanceData);
            case 'piecewiseBilinearBlended':
                return InstancedTileSet.computePiecewiseBilinearBlendedWarp(instanceData);
            case 'piecewiseBilinearSmoothNormal':
                return InstancedTileSet.computePiecewiseBilinearSmoothNormalWarp(instanceData);
            case 'piecewiseBilinearCoonsNormal':
                return InstancedTileSet.computePiecewiseBilinearCoonsNormalWarp(instanceData);
            case 'piecewiseBilinearCornerNormal':
                return InstancedTileSet.computePiecewiseBilinearCornerNormalWarp(instanceData);
            case 'piecewiseBilinearSecantNormal':
                return InstancedTileSet.computePiecewiseBilinearSecantNormalWarp(instanceData);
        }
    }

    protected createMaterial(mat: MeshStandardNodeMaterial, instanceData: InstanceData): MeshStandardNodeMaterial {
        const normalVarying = varyingProperty('vec3', NORMAL_VARYING);
        mat.positionNode = Fn(() => {
            const { position, normal } = this.computeWarp(instanceData);
            normalVarying.assign(normal);
            return position;
        })();
        mat.normalNode = transformNormalToView(normalVarying);
        return mat;
    }

    setTile(key: number, tileValue: number, tiles: TileDistortionLike, matrix: THREE.Matrix4): boolean {
        this._scratch.set(matrix.elements, 0);
        let variantIndex: number | undefined = undefined;
        switch (this.method) {
            case 'trilinear':
                variantIndex = this.resolver.resolveTrilinear(tiles, key, tileValue, this.height, this._scratch, 16);
                break;
            case 'lagrangeQ9':
            case 'coonsBell':
            case 'rationalBezier':
            case 'piecewiseBilinear':
            case 'piecewiseBilinearBlended':
            case 'piecewiseBilinearSmoothNormal':
            case 'piecewiseBilinearCoonsNormal':
            case 'piecewiseBilinearCornerNormal':
            case 'piecewiseBilinearSecantNormal':
                variantIndex = this.resolver.resolveQ9Schema(tiles, key, tileValue, this.height, this._scratch, 16);
                break;
        }
        if (variantIndex === undefined) return false;
        return super.setInstance(key, variantIndex, 0, this._scratch);
    }

    removeTile(key: number): boolean {
        return super.removeInstance(key);
    }
}

function toVariants(modelSet: ModelSet): VariantDef[] {
    return modelSet.models.map((m) => ({
        parts: m.parts.map((p) => ({
            baseMaterial: p.material,
            indexStart: p.indexStart,
            indexEnd: p.indexEnd
        }))
    }));
}
