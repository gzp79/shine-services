import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { color } from 'three/tsl';
import { MeshStandardNodeMaterial, WebGPURenderer } from 'three/webgpu';
import { InstancedTileSet } from '../../engine/nodes/instanced-tile-set';
import type { TileDistortion } from '../../engine/nodes/instanced-tile-set';
import { WireNode } from '../../engine/nodes/wire-node';
import { own, share } from '../../engine/render/ownership';
import { Experiment } from '../experiment';

const TILE_HEIGHT = 80;

function buildProceduralTileSet(parent: THREE.Object3D, instanceCountHint: number): InstancedTileSet {
    const sphereGeo = new THREE.SphereGeometry(0.4, 16, 12);
    sphereGeo.translate(0.5, 0.5, 0.5);
    const boxGeo = new THREE.BoxGeometry(0.7, 0.7, 0.7, 2, 2, 2);
    boxGeo.translate(0.5, 0.5, 0.5);
    const torusGeo = new THREE.TorusGeometry(0.3, 0.12, 12, 24);
    torusGeo.translate(0.5, 0.5, 0.5);

    const geos = [sphereGeo, boxGeo, torusGeo];
    let totalVerts = 0;
    let totalIndices = 0;
    for (const g of geos) {
        totalVerts += g.attributes.position.count;
        totalIndices += g.index!.count;
    }

    const positions = new Float32Array(totalVerts * 3);
    const indices = new Uint32Array(totalIndices);
    const ranges: number[] = [];
    let vOffset = 0;
    let iOffset = 0;

    for (const g of geos) {
        const pos = g.attributes.position.array as Float32Array;
        positions.set(pos, vOffset * 3);
        const src = g.index!.array;
        ranges.push(iOffset);
        for (let i = 0; i < src.length; i++) indices[iOffset + i] = src[i] + vOffset;
        iOffset += src.length;
        ranges.push(iOffset);
        vOffset += g.attributes.position.count;
        g.dispose();
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));

    const makeMat = (hex: number) => {
        const m = new MeshStandardNodeMaterial({ roughness: 0.6, metalness: 0.2, side: THREE.DoubleSide });
        m.colorNode = color(hex);
        return share(m);
    };

    return new InstancedTileSet(parent, {
        geometry: own(geometry),
        variants: [
            { parts: [{ baseMaterial: makeMat(0x4488cc), indexStart: ranges[0], indexEnd: ranges[1] }] },
            { parts: [{ baseMaterial: makeMat(0xcc4444), indexStart: ranges[2], indexEnd: ranges[3] }] },
            { parts: [{ baseMaterial: makeMat(0x44cc88), indexStart: ranges[4], indexEnd: ranges[5] }] }
        ],
        instanceCountHint
    });
}

// CCW quad corners [BL, BR, TR, TL] → trilinear cp indices [0, 1, 3, 2]
// cp layout: (0,0)=cp0, (1,0)=cp1, (0,1)=cp2, (1,1)=cp3  (bottom face, z=0)
//            (0,0)=cp4, (1,0)=cp5, (0,1)=cp6, (1,1)=cp7  (top face,    z=1)
const CCW_TO_CP = [0, 1, 3, 2];

function buildTileDistortion(tileDistortions: Float32Array, tileIdx: number): TileDistortion {
    const d = new Float32Array(24);
    const base = tileIdx * 8; // 4 corners × 2 coords
    for (let c = 0; c < 4; c++) {
        const x = tileDistortions[base + c * 2];
        const y = tileDistortions[base + c * 2 + 1];
        const cp = CCW_TO_CP[c];
        d[cp * 3] = x;
        d[cp * 3 + 1] = y;
        d[cp * 3 + 2] = 0;
        d[(cp + 4) * 3] = x;
        d[(cp + 4) * 3 + 1] = y;
        d[(cp + 4) * 3 + 2] = TILE_HEIGHT;
    }
    return d;
}

export class TileChunk extends Experiment {
    private readonly world: WasmWorld;
    private tileNode: InstancedTileSet;
    private readonly cellsGroup: THREE.Group;
    private readonly fileInput: HTMLInputElement;
    private readonly params = { q: 0, r: 0 };
    private readonly displayParams = { showCells: true };

    private tileCount = 0;
    private tileVariants = new Uint8Array(0);
    private distortions: TileDistortion[] = [];
    private loadedChunk: { q: number; r: number } | null = null;
    private cellWire: WireNode | null = null;

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer, { title: 'Tile Chunk' });

        this.camera.far = 8000;
        this.camera.updateProjectionMatrix();
        this.camera.position.set(0, -1800, 2000);
        this.camera.lookAt(0, 0, 0);
        if (this.controls) this.controls.update();

        this.world = new WasmWorld();
        this.tileNode = buildProceduralTileSet(this.scene, 2048);
        this.cellsGroup = new THREE.Group();
        this.scene.add(this.cellsGroup);

        this.fileInput = document.createElement('input');
        this.fileInput.type = 'file';
        this.fileInput.accept = '.glb';
        this.fileInput.style.display = 'none';
        container.appendChild(this.fileInput);
        this.fileInput.addEventListener('change', (e) => void this.onGltfFileChange(e));

        const gui = this.debugPanel.root();
        const qCtrl = gui
            .add(this.params, 'q')
            .name('Q')
            .step(1)
            .onFinishChange(() => this.regenerate());
        const rCtrl = gui
            .add(this.params, 'r')
            .name('R')
            .step(1)
            .onFinishChange(() => this.regenerate());
        gui.add(
            {
                randomize: () => {
                    const range = 100;
                    this.params.q = Math.floor(Math.random() * 2 * range) - range;
                    this.params.r = Math.floor(Math.random() * 2 * range) - range;
                    qCtrl.updateDisplay();
                    rCtrl.updateDisplay();
                    this.regenerate();
                }
            },
            'randomize'
        ).name('Random Chunk');

        gui.add({ switchRandom: () => this.switchRandomTile() }, 'switchRandom').name('Switch Random Tile');
        gui.add(this.displayParams, 'showCells')
            .name('Show Cells')
            .onChange((v: boolean) => (v ? this.cellWire?.show() : this.cellWire?.hide()));
        gui.add({ loadGltf: () => this.fileInput.click() }, 'loadGltf').name('Load glTF...');
        gui.add(
            {
                clearGltf: () => {
                    this.fileInput.value = '';
                    this.replaceTileSet(buildProceduralTileSet(this.scene, 2048));
                }
            },
            'clearGltf'
        ).name('Clear glTF');

        this.regenerate();
    }

    private async onGltfFileChange(e: Event): Promise<void> {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) {
            this.replaceTileSet(buildProceduralTileSet(this.scene, 2048));
            return;
        }
        try {
            const url = URL.createObjectURL(file);
            const next = await InstancedTileSet.fromGltf(this.scene, url, { instanceCountHint: 2048 });
            URL.revokeObjectURL(url);
            this.replaceTileSet(next);
        } catch (err) {
            console.error('Failed to load glTF:', err);
        }
    }

    private replaceTileSet(next: InstancedTileSet): void {
        this.tileNode.dispose();
        this.tileNode = next;
        // Re-add all current tiles into the new set
        for (let i = 0; i < this.tileCount; i++) {
            const v = i % this.tileNode.variantCount;
            this.tileVariants[i] = v;
            this.tileNode.setTile(v, i, new THREE.Matrix4(), this.distortions[i]);
        }
    }

    private regenerate(): void {
        if (this.loadedChunk) {
            for (let i = 0; i < this.tileCount; i++) {
                this.tileNode.removeTile(this.tileVariants[i], i);
            }
            this.world.remove_chunk(this.loadedChunk.q, this.loadedChunk.r);
            this.loadedChunk = null;
        }
        this.tileCount = 0;
        this.tileVariants = new Uint8Array(0);
        this.distortions = [];

        this.cellWire?.dispose();
        this.cellWire = null;

        const { q, r } = this.params;
        this.world.init_chunk(q, r);
        this.loadedChunk = { q, r };

        const cells = this.world.inner_cells(q, r)!;
        const tileCount = cells.tiles.length;
        const tileDistortions = new Float32Array(cells.tile_distortions);
        const cellVertices = new Float32Array(cells.vertices);
        const cellIndices = new Uint32Array(cells.indices);
        const cellRanges = new Uint32Array(cells.ranges);
        cells.free();

        this.tileCount = tileCount;
        this.tileVariants = new Uint8Array(tileCount).map((_, i) => i % this.tileNode.variantCount);

        for (let i = 0; i < tileCount; i++) {
            const d = buildTileDistortion(tileDistortions, i);
            this.distortions.push(d);
            this.tileNode.setTile(this.tileVariants[i], i, new THREE.Matrix4(), d);
        }
        this.cellWire = WireNode.fromPolygons(this.cellsGroup, {
            vertices: cellVertices,
            indices: cellIndices,
            ranges: cellRanges
        });
        if (this.displayParams.showCells) this.cellWire.show();
    }

    private switchRandomTile(): void {
        if (this.tileCount === 0) return;
        const key = Math.floor(Math.random() * this.tileCount);
        const currentVariant = this.tileVariants[key];
        const nextVariant = (currentVariant + 1) % this.tileNode.variantCount;
        this.tileNode.removeTile(currentVariant, key);
        this.tileNode.setTile(nextVariant, key, new THREE.Matrix4(), this.distortions[key]);
        this.tileVariants[key] = nextVariant;
    }

    dispose(): void {
        this.fileInput.remove();
        if (this.loadedChunk) {
            this.world.remove_chunk(this.loadedChunk.q, this.loadedChunk.r);
        }
        this.cellWire?.dispose();
        this.scene.remove(this.cellsGroup);
        this.tileNode.dispose();
        this.world.free();
        super.dispose();
    }
}
