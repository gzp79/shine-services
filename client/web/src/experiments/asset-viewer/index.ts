import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import type { GltfAsset } from '../../engine/render/asset-loader';
import { loadGltf } from '../../engine/render/asset-loader';
import { Experiment } from '../experiment';

export class AssetViewer extends Experiment {
    private readonly fileInput: HTMLInputElement;
    private asset: GltfAsset | null = null;
    private meshes: THREE.Mesh[] = [];
    private selectedIndex = 0;

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer, { title: 'Asset Viewer' });

        this.fileInput = document.createElement('input');
        this.fileInput.type = 'file';
        this.fileInput.accept = '.glb,.gltf';
        this.fileInput.style.display = 'none';
        container.appendChild(this.fileInput);
        this.fileInput.addEventListener('change', (e) => void this.onFileChange(e));

        const gui = this.debugPanel.root();
        gui.add({ load: () => this.fileInput.click() }, 'load').name('Load asset…');

        this.start();
    }

    private async onFileChange(e: Event): Promise<void> {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;
        try {
            const url = URL.createObjectURL(file);
            const asset = await loadGltf(url);
            URL.revokeObjectURL(url);
            this.setAsset(asset);
        } catch (err) {
            console.error('[AssetViewer] failed to load:', err);
        }
    }

    private setAsset(asset: GltfAsset): void {
        this.clearMeshes();
        this.asset?.geometry.dispose();
        this.asset = asset;
        this.selectedIndex = 0;

        const gui = this.debugPanel.root();
        const existing = gui.controllers.find((c) => c.property === 'mesh');
        if (existing) existing.destroy();

        const names = asset.meshes.map((m) => m.name);
        const proxy = { mesh: names[0] ?? '' };
        gui.add(proxy, 'mesh', names)
            .name('Mesh')
            .onChange((name: string) => {
                const idx = asset.meshes.findIndex((m) => m.name === name);
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
        if (!this.asset) return;
        this.clearMeshes();

        const entry = this.asset.meshes[this.selectedIndex];
        for (const sub of entry.submeshes) {
            const geo = new THREE.BufferGeometry();
            for (const [name, attr] of Object.entries(this.asset.geometry.attributes)) {
                geo.setAttribute(name, attr);
            }
            geo.setIndex(this.asset.geometry.index);
            geo.setDrawRange(sub.indexStart, sub.indexEnd - sub.indexStart);

            const mesh = new THREE.Mesh(geo, sub.material);
            mesh.frustumCulled = false;
            this.scene.add(mesh);
            this.meshes.push(mesh);
        }
    }

    dispose(): void {
        this.fileInput.remove();
        this.clearMeshes();
        this.asset?.geometry.dispose();
        super.dispose();
    }
}
