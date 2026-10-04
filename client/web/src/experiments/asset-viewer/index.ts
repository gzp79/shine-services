import * as THREE from 'three';
import { ASSET_KINDS } from '../../engine/assets/catalog';
import { type ModelSet } from '../../engine/assets/model-set';
import type { SceneContext } from '../../engine/scene';
import { fireAndForget } from '../../engine/utils';
import { AssetSourcePicker } from '../asset-source-picker';
import { Experiment } from '../experiment';

const NORMAL_LINE_LENGTH = 0.1;
const NORMAL_LINE_COLOR = 0xff00ff;

// One line (2 verts) per unique vertex referenced by [indexStart, indexEnd) — the part's own draw
// range — so normals are drawn only for that submesh, not every vertex in the modelSet's shared buffer.
function buildNormalsLineGeometry(
    geometry: THREE.BufferGeometry,
    indexStart: number,
    indexEnd: number
): THREE.BufferGeometry {
    const positions = geometry.attributes.position;
    const normals = geometry.attributes.normal;
    const indices = geometry.index;

    const vertexIds = new Set<number>();
    for (let i = indexStart; i < indexEnd; i++) {
        vertexIds.add(indices ? indices.getX(i) : i);
    }

    const linePositions = new Float32Array(vertexIds.size * 2 * 3);
    let w = 0;
    for (const vi of vertexIds) {
        const px = positions.getX(vi);
        const py = positions.getY(vi);
        const pz = positions.getZ(vi);
        linePositions.set([px, py, pz], w * 6);
        linePositions.set(
            [
                px + normals.getX(vi) * NORMAL_LINE_LENGTH,
                py + normals.getY(vi) * NORMAL_LINE_LENGTH,
                pz + normals.getZ(vi) * NORMAL_LINE_LENGTH
            ],
            w * 6 + 3
        );
        w++;
    }

    const lineGeometry = new THREE.BufferGeometry();
    lineGeometry.setAttribute('position', new THREE.BufferAttribute(linePositions, 3));
    return lineGeometry;
}

export class AssetViewer extends Experiment {
    private modelSet: ModelSet | null = null;
    private meshes: THREE.Mesh[] = [];
    private normalsHelpers: THREE.LineSegments[] = [];
    private selectedIndex = 0;
    private readonly assetPicker: AssetSourcePicker;
    private readonly displayParams = { showNormals: false };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    private meshCtrl: any = null;

    constructor(context: SceneContext) {
        super(context, { title: 'Asset Viewer' });

        const gui = this.debugPanel.root();
        this.assetPicker = new AssetSourcePicker(gui, this.assets, [...ASSET_KINDS], {
            onNone: () => this.clearModel(),
            onAsset: (name) => fireAndForget(this.loadAsset(name)),
            onFile: (url) => fireAndForget(this.loadFile(url))
        });

        gui.add(this.displayParams, 'showNormals')
            .name('Show Normals')
            .onChange((v: boolean) => {
                if (v) this.attachNormalsDebug();
                else this.clearNormalsDebug();
            });
    }

    init(): void {
        this.context.runtime.spawn(this.assetPicker.populate());
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
        this.clearNormalsDebug();
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

        if (this.displayParams.showNormals) this.attachNormalsDebug();
    }

    private attachNormalsDebug(): void {
        this.clearNormalsDebug();
        if (!this.modelSet) return;

        const geometry = this.modelSet.geometry;
        const entry = this.modelSet.models[this.selectedIndex];
        this.normalsHelpers = entry.parts.map((part) => {
            const lineGeo = buildNormalsLineGeometry(geometry, part.indexStart, part.indexEnd);
            const lines = new THREE.LineSegments(lineGeo, new THREE.LineBasicMaterial({ color: NORMAL_LINE_COLOR }));
            lines.frustumCulled = false;
            this.scene.add(lines);
            return lines;
        });
    }

    private clearNormalsDebug(): void {
        for (const lines of this.normalsHelpers) {
            this.scene.remove(lines);
            lines.geometry.dispose();
            (lines.material as THREE.Material).dispose();
        }
        this.normalsHelpers = [];
    }

    dispose(): void {
        this.assetPicker.dispose();
        this.clearMeshes();
        super.dispose();
    }
}
