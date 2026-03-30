import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { ChunkId } from './chunk-id';
import { MeshBuilder, MeshData, buildBaseMesh } from './mesh-builder';

export class Chunk {
    readonly group = new THREE.Group();
    private mesh: MeshBuilder | null = null;
    private label: THREE.Sprite | null = null;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkId
    ) {
        this.group.userData = { chunkId: { q: id.q, r: id.r } };
    }

    get showLabel(): boolean {
        return this.label?.visible ?? false;
    }

    set showLabel(value: boolean) {
        if (value) {
            this.createLabel();
        } else {
            this.destroyLabel();
        }
    }

    buildMesh(reference: ChunkId): void {
        this.disposeMesh();

        const color = new THREE.Color().setHSL(this.chunkHash() / 0xffffffff, 0.5, 0.6);

        const meshData: MeshData = {
            quadVertices: this.quadVertices(),
            quadIndices: this.quadIndices(),
            boundaryIndices: this.boundaryIndices(),
            dualVertices: this.dualVertices(),
            dualIndices: this.dualIndices()
        };
        this.mesh = buildBaseMesh(meshData, color);
        this.group.add(this.mesh.group);

        const offset = this.worldOffset(reference);
        this.group.position.set(offset[0], offset[1], 0);
    }

    quadVertices(): Float32Array {
        return this.world.chunk_quad_vertices(this.id.q, this.id.r);
    }

    quadIndices(): Uint32Array {
        return this.world.chunk_quad_indices(this.id.q, this.id.r);
    }

    boundaryIndices(): Uint32Array {
        return this.world.chunk_boundary_indices(this.id.q, this.id.r);
    }

    dualVertices(): Float32Array {
        return this.world.chunk_dual_vertices(this.id.q, this.id.r);
    }

    dualIndices(): Uint32Array {
        return this.world.chunk_dual_indices(this.id.q, this.id.r);
    }

    worldOffset(ref: ChunkId): Float32Array {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.q, this.id.r);
    }

    /** Simple hash for deterministic debug color. */
    private chunkHash(): number {
        let h = ((this.id.q | 0) * 0x9e3779b9 + (this.id.r | 0)) >>> 0;
        h ^= h >>> 16;
        h = Math.imul(h, 0x45d9f3b) >>> 0;
        h ^= h >>> 16;
        return h;
    }

    disposeMesh(): void {
        if (this.mesh) {
            this.group.remove(this.mesh.group);
            this.mesh.dispose();
            this.mesh = null;
        }
        this.destroyLabel();
    }

    private createLabel(): void {
        if (this.label) return;

        // Create canvas for text
        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d')!;
        canvas.width = 256;
        canvas.height = 128;

        // Draw text
        context.fillStyle = 'rgba(0, 0, 0, 0.7)';
        context.fillRect(0, 0, canvas.width, canvas.height);
        context.font = 'bold 48px monospace';
        context.fillStyle = 'white';
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        context.fillText(`(${this.id.q}, ${this.id.r})`, canvas.width / 2, canvas.height / 2);

        // Create sprite
        const texture = new THREE.CanvasTexture(canvas);
        const material = new THREE.SpriteMaterial({
            map: texture,
            depthTest: false,
            depthWrite: false
        });
        this.label = new THREE.Sprite(material);
        this.label.scale.set(200, 100, 1); // Scale in world units
        this.label.position.set(0, 0, 50); // Position above chunk center
        this.label.renderOrder = 998; // Render before centerDot
        this.group.add(this.label);
    }

    private destroyLabel(): void {
        if (!this.label) return;

        this.group.remove(this.label);
        const material = this.label.material as THREE.SpriteMaterial;
        material.map?.dispose();
        material.dispose();
        this.label = null;
    }
}
