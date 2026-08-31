import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import type { AssetCatalogBuilder } from '../../engine/assets/catalog';
import { type ModelSet } from '../../engine/assets/model-set';
import { AssetSourcePicker } from '../asset-source-picker';
import { Experiment } from '../experiment';

export class AssetViewer extends Experiment {
    private modelSet: ModelSet | null = null;
    private meshes: THREE.Mesh[] = [];
    private selectedIndex = 0;
    private readonly assetPicker: AssetSourcePicker;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    private meshCtrl: any = null;

    constructor(container: HTMLElement, renderer: WebGPURenderer, catalogBuilder: AssetCatalogBuilder) {
        super(container, renderer, { title: 'Asset Viewer' }, catalogBuilder);

        this.assetPicker = new AssetSourcePicker(this.debugPanel.root(), this.assets, {
            onNone: () => this.clearModel(),
            onAsset: (name) => void this.loadAsset(name),
            onFile: (url) => void this.loadFile(url)
        });
        void this.assetPicker.populate();

        this.start();
    }

    private async loadFile(url: string): Promise<void> {
        try {
            this.setModelSet(await this.assets.loadModelSetFromUrl(url));
        } catch (err) {
            console.error('[AssetViewer] failed to load file:', err);
        }
    }

    private async loadAsset(name: string): Promise<void> {
        try {
            this.setModelSet(await this.assets.loadModelSet(name));
        } catch (err) {
            console.error(`[AssetViewer] failed to load "${name}":`, err);
        }
    }

    private clearModel(): void {
        this.clearMeshes();
        this.modelSet = null;
        this.selectedIndex = 0;
        this.meshCtrl?.destroy();
        this.meshCtrl = null;
    }

    private setModelSet(modelSet: ModelSet): void {
        this.clearMeshes();
        this.modelSet = modelSet;
        this.selectedIndex = 0;

        const gui = this.debugPanel.root();
        this.meshCtrl?.destroy();

        const names = modelSet.models.map((m) => m.name);
        const proxy = { mesh: names[0] ?? '' };
        this.meshCtrl = gui
            .add(proxy, 'mesh', names)
            .name('Mesh')
            .onChange((name: string) => {
                const idx = modelSet.models.findIndex((m) => m.name === name);
                if (idx >= 0) {
                    this.selectedIndex = idx;
                    this.showMesh();
                }
            });

        this.showMesh();
    }

    private clearMeshes(): void {
        for (const m of this.meshes) {
            this.scene.remove(m);
            m.geometry.dispose();
        }
        this.meshes = [];
    }

    private showMesh(): void {
        if (!this.modelSet) return;
        this.clearMeshes();

        const entry = this.modelSet.models[this.selectedIndex];
        for (const part of entry.parts) {
            const geo = new THREE.BufferGeometry();
            for (const [name, attr] of Object.entries(this.modelSet.geometry.attributes)) {
                geo.setAttribute(name, attr);
            }
            geo.setIndex(this.modelSet.geometry.index);
            geo.setDrawRange(part.indexStart, part.indexEnd - part.indexStart);

            const mesh = new THREE.Mesh(geo, part.material);
            mesh.frustumCulled = false;
            this.scene.add(mesh);
            this.meshes.push(mesh);
        }
    }

    dispose(): void {
        this.assetPicker.dispose();
        this.clearMeshes();
        super.dispose();
    }
}
