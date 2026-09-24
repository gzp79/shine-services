import { ChangeLog, InnerCells, World } from '#wasm';
import * as THREE from 'three';
import { ReadonlyBitSet, asBitSet } from '../../bit-set';
import type { SceneContext } from '../../engine/scene';
import { InstancedTileSet } from '../../engine/scene/instancing/instanced-tile-set';
import type { TileDistortion } from '../../engine/scene/instancing/instanced-tile-set';
import { WireMesh } from '../../engine/scene/wire-mesh';
import { fireAndForget } from '../../engine/utils';
import { asPolygonMesh, asTileOutlineMesh } from '../../mesh/polygon-mesh';
import { AssetSourcePicker } from '../asset-source-picker';
import { Experiment } from '../experiment';
import { QuadrantLabels } from './quadrant-labels';

const TILE_HEIGHT = 80;
const INSTANCE_COUNT_HINT = 2048;
const INITIAL_ASSET = 'generated-shapes';

// CCW quad corners [BL, BR, TR, TL] → trilinear cp indices [0, 1, 3, 2]
// cp layout: (0,0)=cp0, (1,0)=cp1, (0,1)=cp2, (1,1)=cp3  (bottom face, z=0)
//            (0,0)=cp4, (1,0)=cp5, (0,1)=cp6, (1,1)=cp7  (top face,    z=1)
const CCW_TO_CP = [0, 1, 3, 2];

/** Maps a tile's base-layer value to which variant of the loaded tile set should render it. */
function tileVariant(value: number, variantCount: number): number {
    return value % variantCount;
}

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
    private readonly world: World;
    private tileNode: InstancedTileSet | null = null;
    private readonly chunkGroup: THREE.Group;
    private readonly assetPicker: AssetSourcePicker;
    private readonly params = { q: 0, r: 0 };
    private readonly displayParams = { showMeshes: true, showCells: true, showTiles: false, showQuadrants: false };

    private tileCount = 0;
    private distortions: TileDistortion[] = [];
    private loadedChunk: { q: number; r: number } | null = null;
    private innerCells: InnerCells | null = null;
    private cellWire: WireMesh | null = null;
    private tileWire: WireMesh | null = null;
    private quadrantLabels: QuadrantLabels | null = null;
    private variantVisible: boolean[] = [];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    private variantVisibleFolder: any = null;

    constructor(context: SceneContext) {
        super(context, { title: 'Tile Chunk' });

        this.camera.far = 8000;
        this.camera.updateProjectionMatrix();
        this.camera.position.set(0, -1800, 2000);
        this.camera.lookAt(0, 0, 0);
        if (this.controls) this.controls.update();

        this.world = new World();
        this.chunkGroup = new THREE.Group();
        this.scene.add(this.chunkGroup);

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

        gui.add({ switchRandom: () => this.switchRandomCell() }, 'switchRandom').name('Switch Random Cell');
        gui.add({ sync: () => this.syncBaseLayer() }, 'sync').name('Sync Base Layer');
        gui.add(this.displayParams, 'showMeshes')
            .name('Show Meshes')
            .onChange((v: boolean) => {
                if (this.tileNode) this.tileNode.group.visible = v;
            });
        gui.add(this.displayParams, 'showCells')
            .name('Show Cells')
            .onChange((v: boolean) => (v ? this.cellWire?.show() : this.cellWire?.hide()));
        gui.add(this.displayParams, 'showTiles')
            .name('Show Tiles')
            .onChange((v: boolean) => (v ? this.tileWire?.show() : this.tileWire?.hide()));
        gui.add(this.displayParams, 'showQuadrants')
            .name('Show Quadrants')
            .onChange((v: boolean) => (v ? this.quadrantLabels?.show() : this.quadrantLabels?.hide()));

        this.assetPicker = new AssetSourcePicker(gui, this.assets, {
            onNone: () => this.replaceTileSet(undefined),
            onAsset: (name) => fireAndForget(this.loadAsset(name)),
            onFile: (url) => fireAndForget(this.loadFile(url))
        });

        this.rebuildVariantVisibilityFolder();
    }

    init(): void {
        this.context.runtime.spawn(this.assetPicker.populate());
        this.context.runtime.spawn(this.loadAsset(INITIAL_ASSET));
        this.regenerate();
    }

    private async loadAsset(name: string): Promise<void> {
        try {
            const modelSet = await this.assets.loadModelSet(name);
            const next = InstancedTileSet.fromModelSet(this.chunkGroup, modelSet, {
                instanceCountHint: INSTANCE_COUNT_HINT
            });
            this.replaceTileSet(next);
        } catch (err) {
            console.error(`[TileChunk] failed to load asset "${name}":`, err);
        }
    }

    private async loadFile(url: string): Promise<void> {
        try {
            const next = await InstancedTileSet.fromGltf(this.chunkGroup, url, {
                instanceCountHint: INSTANCE_COUNT_HINT
            });
            this.replaceTileSet(next);
        } catch (err) {
            console.error('Failed to load glTF:', err);
        }
    }

    // `undefined` drops the current tile set: the chunk keeps its cells/wires but renders no tiles.
    private replaceTileSet(next: InstancedTileSet | undefined): void {
        this.tileNode?.dispose();
        this.tileNode = next ?? null;
        this.rebuildVariantVisibilityFolder();
        if (this.tileNode) {
            this.tileNode.group.visible = this.displayParams.showMeshes;
            this.syncBaseLayer(true);
        }
    }

    private regenerate(): void {
        if (this.loadedChunk) {
            this.tileNode?.removeAll();
            this.world.remove_chunk(this.loadedChunk.q, this.loadedChunk.r);
            this.loadedChunk = null;
        }
        this.tileCount = 0;
        this.distortions = [];

        this.cellWire?.dispose();
        this.cellWire = null;
        this.tileWire?.dispose();
        this.tileWire = null;
        this.quadrantLabels?.dispose();
        this.quadrantLabels = null;
        this.innerCells?.free();
        this.innerCells = null;

        const { q, r } = this.params;
        this.world.init_chunk(q, r);
        this.loadedChunk = { q, r };

        this.innerCells = this.world.inner_cells(q, r)!;
        const tileCount = this.innerCells.tile_ids()!.length;
        const tileDistortions = this.innerCells.tile_distortions()!;

        this.tileCount = tileCount;

        // Distortions are kept regardless so a later asset load can bind tiles; render state comes from the sync below.
        for (let i = 0; i < tileCount; i++) {
            this.distortions.push(buildTileDistortion(tileDistortions, i));
        }
        this.cellWire = WireMesh.fromPolygons(this.chunkGroup, asPolygonMesh(this.innerCells));
        if (this.displayParams.showCells) this.cellWire.show();

        this.tileWire = WireMesh.fromPolygons(this.chunkGroup, asTileOutlineMesh(this.innerCells), {
            color: 0xffaa00
        });
        if (this.displayParams.showTiles) this.tileWire.show();

        // A fresh chunk's base layer is all zero, so labels start zeroed too; created once, like the wires,
        // and merely shown/hidden afterwards rather than rebuilt. The sync below corrects the text either way.
        this.quadrantLabels = new QuadrantLabels(this.chunkGroup, this.innerCells, new Uint32Array(tileCount));
        if (this.displayParams.showQuadrants) this.quadrantLabels.show();
        else this.quadrantLabels.hide();

        this.syncBaseLayer(true);
    }

    private syncBaseLayer(forced = false): void {
        if (!this.loadedChunk) return;
        using changeLog = this.world.sync_base_layer(this.loadedChunk.q, this.loadedChunk.r);
        if (!changeLog) return;
        this.consumeLog(changeLog, forced);
    }

    private consumeLog(changeLog: ChangeLog, forced: boolean): void {
        const values = changeLog.values();
        const apply = (tileIdx: number): void => {
            const value = values[tileIdx]!;
            if (this.tileNode) {
                const variant = tileVariant(value, this.tileNode.variantCount);
                this.tileNode.setTile(tileIdx, variant, new THREE.Matrix4(), this.distortions[tileIdx]!);
            }
            this.quadrantLabels?.updateTileValue(tileIdx, value);
        };

        if (forced) {
            for (let i = 0; i < this.tileCount; i++) apply(i);
        } else {
            new ReadonlyBitSet(asBitSet(changeLog)).forEachSet(apply);
        }
    }

    private switchRandomCell(): void {}

    private rebuildVariantVisibilityFolder(): void {
        const gui = this.debugPanel.root();
        if (this.variantVisibleFolder) {
            this.variantVisibleFolder.destroy();
            this.variantVisibleFolder = null;
        }

        const count = this.tileNode?.variantCount ?? 0;
        this.variantVisible = Array.from({ length: count }, (_, i) =>
            i < this.variantVisible.length ? this.variantVisible[i] : true
        );

        const folder = gui.addFolder('Variants');
        folder.close();
        this.variantVisibleFolder = folder;

        const allParam = { showAll: this.variantVisible.every((v) => v) };
        const allCtrl = folder.add(allParam, 'showAll').name('Show All');

        const row = document.createElement('div');
        row.style.cssText =
            'display:flex;gap:8px;padding:0 var(--padding);height:var(--widget-height);align-items:center;';

        const checkboxes = this.variantVisible.map((checked, i) => {
            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.checked = checked;
            checkbox.style.cssText = 'cursor:pointer;';
            checkbox.addEventListener('change', () => {
                this.variantVisible[i] = checkbox.checked;
                allParam.showAll = this.variantVisible.every((v) => v);
                allCtrl.updateDisplay();
                this.tileNode?.setVariantVisible(i, checkbox.checked);
            });
            const label = document.createElement('label');
            label.textContent = String(i);
            label.style.cssText = 'display:flex;gap:4px;align-items:center;cursor:pointer;';
            label.prepend(checkbox);
            row.appendChild(label);
            return checkbox;
        });

        (folder as unknown as { $children: HTMLElement }).$children.appendChild(row);

        allCtrl.onChange((value: boolean) => {
            this.variantVisible.fill(value);
            checkboxes.forEach((cb) => (cb.checked = value));
            for (let i = 0; i < count; i++) this.tileNode?.setVariantVisible(i, value);
        });
    }

    dispose(): void {
        this.assetPicker.dispose();
        if (this.loadedChunk) {
            this.world.remove_chunk(this.loadedChunk.q, this.loadedChunk.r);
        }
        this.innerCells?.free();
        this.cellWire?.dispose();
        this.tileWire?.dispose();
        this.quadrantLabels?.dispose();
        this.scene.remove(this.chunkGroup);
        this.tileNode?.dispose();
        this.world.free();
        super.dispose();
    }
}
