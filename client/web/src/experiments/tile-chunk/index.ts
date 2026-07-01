import init, { WasmWorld } from '#wasm';
import wasmUrl from '#wasm-bin';
import GUI from 'lil-gui';
import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import { TileSetNode } from '../../engine/nodes/tile-set-node';
import type { TileDistortion } from '../../engine/nodes/tile-set-node';
import { WireNode } from '../../engine/nodes/wire-node';
import { Experiment } from '../experiment';

export interface TileChunkExperiment {
    dispose(): void;
}

const TILE_HEIGHT = 80;

function buildCombinedMesh(): { geometry: THREE.BufferGeometry; ranges: Uint32Array } {
    const boxGeo = new THREE.BoxGeometry(0.8, 0.8, 0.8, 4, 4, 4);
    boxGeo.translate(0.5, 0.5, 0.5);
    const boxPos = new Float32Array(boxGeo.attributes.position.array);
    const boxIdx = new Uint32Array(boxGeo.index!.array);
    const boxVertCount = boxPos.length / 3;

    const sphereGeo = new THREE.SphereGeometry(0.5, 16, 12);
    sphereGeo.translate(0.5, 0.5, 0.5);
    const spherePos = new Float32Array(sphereGeo.attributes.position.array);
    const sphereIdxRaw = sphereGeo.index!.array;
    const sphereIdx = new Uint32Array(sphereIdxRaw.length);
    for (let i = 0; i < sphereIdxRaw.length; i++) {
        sphereIdx[i] = sphereIdxRaw[i] + boxVertCount;
    }

    const vertices = new Float32Array(boxPos.length + spherePos.length);
    vertices.set(boxPos, 0);
    vertices.set(spherePos, boxPos.length);

    const indices = new Uint32Array(boxIdx.length + sphereIdx.length);
    indices.set(boxIdx, 0);
    indices.set(sphereIdx, boxIdx.length);

    boxGeo.dispose();
    sphereGeo.dispose();

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(vertices, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));

    // range 0 = box, range 1 = sphere
    const ranges = new Uint32Array([0, boxIdx.length, boxIdx.length, indices.length]);

    return { geometry, ranges };
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

class TileChunk extends Experiment {
    private readonly world: WasmWorld;
    private readonly tileNode: TileSetNode;
    private readonly cellsGroup: THREE.Group;
    private readonly gui: GUI;
    private readonly params = { q: 0, r: 0 };
    private readonly displayParams = { showCells: true };

    private tileCount = 0;
    private tileAssignments = new Uint8Array(0); // 0=box, 1=sphere per tile
    private distortions: TileDistortion[] = [];
    private loadedChunk: { q: number; r: number } | null = null;
    private cellWire: WireNode | null = null;

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer);

        this.camera.far = 8000;
        this.camera.updateProjectionMatrix();
        this.camera.position.set(0, -1800, 2000);
        this.camera.lookAt(0, 0, 0);
        if (this.controls) this.controls.update();

        this.world = new WasmWorld();

        this.tileNode = new TileSetNode(this.scene, { ...buildCombinedMesh(), maxInstances: 2048 });

        this.cellsGroup = new THREE.Group();
        this.scene.add(this.cellsGroup);

        this.gui = new GUI({ title: 'Tile Chunk', container });
        this.gui.domElement.style.cssText = 'position:absolute;top:0;right:0;z-index:10';

        const chunkFolder = this.gui.addFolder('Chunk');
        const qCtrl = chunkFolder
            .add(this.params, 'q')
            .name('Q')
            .step(1)
            .onFinishChange(() => this.regenerate());
        const rCtrl = chunkFolder
            .add(this.params, 'r')
            .name('R')
            .step(1)
            .onFinishChange(() => this.regenerate());
        chunkFolder
            .add(
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
            )
            .name('Random Chunk');

        this.gui.add({ switchRandom: () => this.switchRandomTile() }, 'switchRandom').name('Switch Random Tile');

        this.gui
            .add(this.displayParams, 'showCells')
            .name('Show Cells')
            .onChange((v: boolean) => (v ? this.cellWire?.show() : this.cellWire?.hide()));

        this.regenerate();
        this.start();
    }

    private regenerate(): void {
        if (this.loadedChunk) {
            for (let i = 0; i < this.tileCount; i++) {
                this.tileNode.removeInstance(this.tileAssignments[i], i);
            }
            this.world.remove_chunk(this.loadedChunk.q, this.loadedChunk.r);
            this.loadedChunk = null;
        }
        this.tileCount = 0;
        this.tileAssignments = new Uint8Array(0);
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
        this.tileAssignments = new Uint8Array(tileCount).map(() => (Math.random() < 0.5 ? 0 : 1));

        for (let i = 0; i < tileCount; i++) {
            const d = buildTileDistortion(tileDistortions, i);
            this.distortions.push(d);
            if (!this.tileNode.setTile(this.tileAssignments[i], i, d)) {
                console.warn(`setTile failed for key=${i} range=${this.tileAssignments[i]}`);
            }
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
        const currentRange = this.tileAssignments[key];
        const nextRange = currentRange === 0 ? 1 : 0;
        this.tileNode.removeInstance(currentRange, key);
        this.tileNode.setTile(nextRange, key, this.distortions[key]);
        this.tileAssignments[key] = nextRange;
    }

    dispose(): void {
        this.gui.destroy();
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

export async function createTileChunkExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer
): Promise<TileChunkExperiment> {
    await init(wasmUrl);
    return new TileChunk(container, renderer);
}
