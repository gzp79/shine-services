import { BaseLayerOp, InnerCells, TileGeometries, World } from '#wasm';
import * as THREE from 'three';
import { ReadonlyBitSet, asBitSet } from '../../bit-set';
import { type ModelSet, toModelSet } from '../../engine/assets/model-set';
import { loadGltf } from '../../engine/loaders/gltf-loader';
import type { SceneContext } from '../../engine/scene';
import { InstancedNormalLineAttachment } from '../../engine/scene/instancing/instanced-normal-line-attachment';
import {
    InstancedTileSet,
    TILE_DISTORTION_METHODS,
    type TileDistortionMethod
} from '../../engine/scene/instancing/instanced-tile-set';
import { WireMesh } from '../../engine/scene/wire-mesh';
import { fireAndForget } from '../../engine/utils';
import { asPolygonMesh, asTileDistortion, asTileOutlineMesh } from '../../mesh/polygon-mesh';
import { AssetSourcePicker } from '../asset-source-picker';
import { Experiment } from '../experiment';
import { QuadrantLabels } from './quadrant-labels';

const TILE_HEIGHT = 80;
const INSTANCE_COUNT_HINT = 2048;
const INITIAL_ASSET = 'generated-tile-hires';
const INITIAL_FILLED_CELL_COUNT = 30;

export class TileChunk extends Experiment {
    private readonly world: World;
    private tileNode: InstancedTileSet | null = null;
    private readonly chunkGroup: THREE.Group;
    private readonly assetPicker: AssetSourcePicker;
    private readonly params: { q: number; r: number; method: TileDistortionMethod } = {
        q: 0,
        r: 0,
        method: TILE_DISTORTION_METHODS[0]
    };
    private currentModelSet: ModelSet | null = null;
    private readonly displayParams = {
        showMeshes: true,
        showCells: true,
        showTiles: false,
        showQuadrants: false,
        showNormals: false
    };

    private tileCount = 0;
    private loadedChunk: { q: number; r: number } | null = null;
    private innerCells: InnerCells | null = null;
    private tileGeometries: TileGeometries | null = null;
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

        gui.add(this.params, 'method', [...TILE_DISTORTION_METHODS])
            .name('Distortion Method')
            .onChange(() => this.replaceTileSet(this.currentModelSet));

        gui.add({ random: () => this.switchRandomCell() }, 'random').name('Switch Random Cell');
        gui.add({ clear: () => this.updateBaseLayer({ op: 'clear', value: 0 }) }, 'clear').name('Clear');
        gui.add({ fill: () => this.updateBaseLayer({ op: 'clear', value: 1 }) }, 'fill').name('Fill');
        gui.add({ sync: () => this.updateBaseLayer({ op: 'sync' }) }, 'sync').name('Sync Base Layer');
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
        gui.add(this.displayParams, 'showNormals')
            .name('Show Normals')
            .onChange((v: boolean) => {
                if (v) this.attachNormalsDebug();
                else this.tileNode?.detach('normals');
            });

        this.assetPicker = new AssetSourcePicker(gui, this.assets, ['tile-3d'], {
            onNone: () => {
                this.replaceTileSet(null);
            },
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
            this.replaceTileSet(modelSet);
        } catch (err) {
            console.error(`[TileChunk] failed to load asset "${name}":`, err);
        }
    }

    private async loadFile(url: string): Promise<void> {
        try {
            const modelSet = toModelSet(await loadGltf(url), 'owned');
            this.replaceTileSet(modelSet);
        } catch (err) {
            console.error('Failed to load glTF:', err);
        }
    }

    // `null` drops the current tile set: the chunk keeps its cells/wires but renders no tiles.
    private replaceTileSet(modelSet: ModelSet | null): void {
        this.tileNode?.dispose(); // cascades to the attached 'normals' debug helper, if any
        this.tileNode = null;
        this.rebuildVariantVisibilityFolder();
        this.currentModelSet = modelSet;
        if (this.currentModelSet) {
            this.tileNode = InstancedTileSet.fromModelSet(this.chunkGroup, this.currentModelSet, {
                instanceCountHint: INSTANCE_COUNT_HINT,
                height: TILE_HEIGHT,
                method: this.params.method
            });
            this.tileNode.group.visible = this.displayParams.showMeshes;
            if (this.displayParams.showNormals) this.attachNormalsDebug();
            this.updateBaseLayer({ op: 'sync' }, true);
        }
    }

    private attachNormalsDebug(): void {
        this.tileNode?.attach('normals', new InstancedNormalLineAttachment());
    }

    private regenerate(): void {
        if (this.loadedChunk) {
            this.tileNode?.removeAll();
            this.world.remove_chunk(this.loadedChunk.q, this.loadedChunk.r);
            this.loadedChunk = null;
        }
        this.tileCount = 0;

        this.cellWire?.dispose();
        this.cellWire = null;
        this.tileWire?.dispose();
        this.tileWire = null;
        this.quadrantLabels?.dispose();
        this.quadrantLabels = null;
        this.innerCells?.free();
        this.innerCells = null;
        this.tileGeometries?.free();
        this.tileGeometries = null;

        const { q, r } = this.params;
        this.world.init_chunk(q, r);
        this.loadedChunk = { q, r };

        this.innerCells = this.world.inner_cells(q, r)!;
        this.tileGeometries = this.world.tile_geometries(q, r)!;
        const tileCount = this.tileGeometries.tile_count()!;

        this.tileCount = tileCount;

        this.cellWire = WireMesh.fromPolygons(this.chunkGroup, asPolygonMesh(this.innerCells));
        if (this.displayParams.showCells) this.cellWire.show();

        this.tileWire = WireMesh.fromPolygons(this.chunkGroup, asTileOutlineMesh(this.tileGeometries), {
            color: 0xffaa00
        });
        if (this.displayParams.showTiles) this.tileWire.show();

        // A fresh chunk's base layer is all zero, so labels start zeroed too; created once, like the wires,
        // and merely shown/hidden afterwards rather than rebuilt. The sync below corrects the text either way.
        this.quadrantLabels = new QuadrantLabels(this.chunkGroup, this.tileGeometries, new Uint32Array(tileCount));
        if (this.displayParams.showQuadrants) this.quadrantLabels.show();
        else this.quadrantLabels.hide();

        this.fillRandomCells(INITIAL_FILLED_CELL_COUNT);
        this.updateBaseLayer({ op: 'sync' }, true);
    }

    private updateBaseLayer(op: BaseLayerOp, forcedRefresh = false): void {
        if (!this.loadedChunk) return;
        using changeLog = this.world.update_base_layer(this.loadedChunk.q, this.loadedChunk.r, op);
        if (!changeLog) return;

        const values = changeLog.values();
        const tiles = this.tileGeometries ? asTileDistortion(this.tileGeometries) : null;
        const apply = (tileIdx: number): void => {
            const value = values[tileIdx]!;
            if (tiles) this.tileNode?.setTile(tileIdx, value, tiles, new THREE.Matrix4());
            this.quadrantLabels?.updateTileValue(tileIdx, value);
        };

        if (forcedRefresh) {
            for (let i = 0; i < this.tileCount; i++) apply(i);
        } else {
            new ReadonlyBitSet(asBitSet(changeLog)).forEachSet(apply);
        }
    }

    private switchRandomCell(): void {
        this.fillRandomCells(10);
    }

    private fillRandomCells(count: number): void {
        if (!this.innerCells) return;
        for (let i = 0; i < count; i++) {
            const cellIds = this.innerCells.cell_ids();
            if (!cellIds || cellIds.length === 0) continue;

            const cell = cellIds[Math.floor(Math.random() * cellIds.length)]!;
            const value = 1; //Math.round(Math.random());
            this.updateBaseLayer({ op: 'setCell', cell, value });
        }
    }

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
        this.tileGeometries?.free();
        this.cellWire?.dispose();
        this.tileWire?.dispose();
        this.quadrantLabels?.dispose();
        this.scene.remove(this.chunkGroup);
        this.tileNode?.dispose(); // cascades to the attached 'normals' debug helper, if any
        this.world.free();
        super.dispose();
    }
}
