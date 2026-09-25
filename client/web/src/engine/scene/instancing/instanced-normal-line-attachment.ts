import * as THREE from 'three';
import { attribute } from 'three/tsl';
import { LineBasicNodeMaterial, type Node } from 'three/webgpu';
import { COLOR_INSTANCE_SCHEMA, InstancedColorMesh } from './instanced-color-mesh';
import {
    type InstanceData,
    type InstancedMeshAttachment,
    type InstancedMultiMesh,
    type SubMeshDef
} from './instanced-multi-mesh';
import { InstancedTileSet, TILE_INSTANCE_SCHEMA } from './instanced-tile-set';

const DEFAULT_LINE_LENGTH = 10;
const DEFAULT_LINE_COLOR = 0xff00ff;

export type InstancedNormalLineAttachmentOptions = {
    readonly lineLength?: number;
    readonly lineColor?: THREE.ColorRepresentation;
};

/**
 * Opt-in debug visualization for an InstancedMultiMesh: one line per unique vertex, from its
 * warped position along its warped normal. Works with any target whose InstanceData.schema is
 * handled by _computeWarp() below.
 */
export class InstancedNormalLineAttachment implements InstancedMeshAttachment {
    private readonly group = new THREE.Group();
    private readonly meshes: (THREE.LineSegments | undefined)[] = [];
    private readonly lineLength: number;
    private readonly lineColor: THREE.ColorRepresentation;

    constructor(options?: InstancedNormalLineAttachmentOptions) {
        this.lineLength = options?.lineLength ?? DEFAULT_LINE_LENGTH;
        this.lineColor = options?.lineColor ?? DEFAULT_LINE_COLOR;
    }

    attach(target: InstancedMultiMesh): void {
        target.group.add(this.group);

        for (let variantIndex = 0; variantIndex < target.variantCount; variantIndex++) {
            this._attachVariant(target, variantIndex);
        }
    }

    private _attachVariant(target: InstancedMultiMesh, variantIndex: number): void {
        const instanceData = target.getInstanceData(variantIndex);
        const parts = target.getVariantParts(variantIndex);
        if (!instanceData || !parts) return;

        const geometry = this._buildLineGeometry(target, parts);
        if (!geometry) return;

        const mesh = new THREE.LineSegments(geometry, this._buildMaterial(instanceData));
        mesh.frustumCulled = false;
        mesh.onBeforeRender = () => {
            geometry.instanceCount = target.instanceCount(variantIndex);
        };
        this.group.add(mesh);
        this.meshes[variantIndex] = mesh;
    }

    // The instance texture our material's shader reads is captured at build time, so a grow
    // (reallocation) leaves it pointing at a stale/disposed texture — rebuild against the new one.
    instanceDataChanged(target: InstancedMultiMesh, variantIndex: number): void {
        const mesh = this.meshes[variantIndex];
        const instanceData = target.getInstanceData(variantIndex);
        if (!mesh || !instanceData) return;
        (mesh.material as THREE.Material).dispose();
        mesh.material = this._buildMaterial(instanceData);
    }

    // Position and normal after whatever per-instance warp the target mesh applies, dispatched by
    // InstanceData.schema so this attachment works with any InstancedMultiMesh subclass.
    private _buildMaterial(instanceData: InstanceData): LineBasicNodeMaterial {
        let position: Node<'vec3'>;
        let normal: Node<'vec3'>;
        switch (instanceData.schema) {
            case COLOR_INSTANCE_SCHEMA:
                ({ position, normal } = InstancedColorMesh.computeWarp(instanceData));
                break;
            case TILE_INSTANCE_SCHEMA:
                ({ position, normal } = InstancedTileSet.computeWarp(instanceData));
                break;
            default:
                throw new Error('InstancedNormalLineAttachment: unsupported InstanceData.schema');
        }

        const lineEnd = attribute<'float'>('lineEnd', 'float');
        const mat = new LineBasicNodeMaterial({ color: this.lineColor, toneMapped: false });
        mat.positionNode = position.add(normal.mul(lineEnd).mul(this.lineLength));
        return mat;
    }

    // One line (2 verts) per unique vertex referenced by `parts`' index ranges. Duplicate lines
    // for vertices shared across parts/triangles are harmless overdraw, not worth deduping here.
    private _buildLineGeometry(
        target: InstancedMultiMesh,
        parts: readonly SubMeshDef[]
    ): THREE.InstancedBufferGeometry | null {
        const src = target.geometry;
        const positions = src.attributes.position;
        const normals = src.attributes.normal;
        if (!positions || !normals) return null;

        const indices = src.index;
        const vertexIds = new Set<number>();
        for (const part of parts) {
            for (let i = part.indexStart; i < part.indexEnd; i++) {
                vertexIds.add(indices ? indices.getX(i) : i);
            }
        }
        if (vertexIds.size === 0) return null;

        const linePositions = new Float32Array(vertexIds.size * 2 * 3);
        const lineNormals = new Float32Array(vertexIds.size * 2 * 3);
        const lineEnds = new Float32Array(vertexIds.size * 2);
        let w = 0;
        for (const vi of vertexIds) {
            for (let end = 0; end < 2; end++, w++) {
                linePositions[w * 3] = positions.getX(vi);
                linePositions[w * 3 + 1] = positions.getY(vi);
                linePositions[w * 3 + 2] = positions.getZ(vi);
                lineNormals[w * 3] = normals.getX(vi);
                lineNormals[w * 3 + 1] = normals.getY(vi);
                lineNormals[w * 3 + 2] = normals.getZ(vi);
                lineEnds[w] = end;
            }
        }

        const geometry = new THREE.InstancedBufferGeometry();
        geometry.setAttribute('position', new THREE.BufferAttribute(linePositions, 3));
        geometry.setAttribute('normal', new THREE.BufferAttribute(lineNormals, 3));
        // indicator for line start/end. 0 for start, 1 for end.
        geometry.setAttribute('lineEnd', new THREE.BufferAttribute(lineEnds, 1));
        geometry.instanceCount = 0;
        return geometry;
    }

    dispose(): void {
        for (const mesh of this.meshes) {
            if (!mesh) continue;
            mesh.geometry.dispose();
            (mesh.material as THREE.Material).dispose();
        }
        this.meshes.length = 0;
        this.group.parent?.remove(this.group);
    }
}
