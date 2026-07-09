import init, { generate_cdt } from '#wasm';
import wasmUrl from '#wasm-bin';
import { WebGPURenderer } from 'three/webgpu';
import { span } from '../../engine/utils';
import type { WasmCdtMesh } from '../../wasm-types/shine_game';
import { Experiment } from '../experiment';
import { cdtParamsToJson, createCdtControls, defaultCdtParams } from './controls';
import { CdtMeshGroup, buildCdtMesh, buildCircumcenterMesh } from './mesh-builder';

export interface CdtExperiment {
    dispose(): void;
}

class Cdt extends Experiment {
    private params = defaultCdtParams();
    private currentCdtHandle: WasmCdtMesh | null = null;
    private currentMesh: CdtMeshGroup | null = null;
    private circumcenterMesh: CdtMeshGroup | null = null;
    private activeTriangleIndex = -1;
    private readonly onKeyDown: (e: KeyboardEvent) => void;

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer, { title: 'CDT' });

        this.camera.near = 1;
        this.camera.far = 50000;
        this.camera.position.set(0, -7000, 12000);
        this.camera.lookAt(0, 0, 0);
        this.camera.updateProjectionMatrix();
        this.controls?.update();

        this.onKeyDown = (e: KeyboardEvent) => {
            if (e.key === '+' || e.key === '=') {
                this.activeTriangleIndex++;
                this.updateCircumcenter();
            } else if (e.key === '-' || e.key === '_') {
                if (this.activeTriangleIndex >= 0) {
                    this.activeTriangleIndex--;
                    this.updateCircumcenter();
                }
            }
        };
        window.addEventListener('keydown', this.onKeyDown);

        createCdtControls(this.debugPanel, this.params, () => this.regenerate());
        this.regenerate();
        this.start();
    }

    private updateCircumcenter() {
        if (this.circumcenterMesh) {
            this.scene.remove(this.circumcenterMesh.group);
            this.circumcenterMesh.dispose();
            this.circumcenterMesh = null;
        }

        if (this.currentCdtHandle && this.activeTriangleIndex >= 0) {
            const triCount = this.currentCdtHandle.triangles().length / 3;
            if (this.activeTriangleIndex < triCount) {
                this.circumcenterMesh = buildCircumcenterMesh(this.currentCdtHandle, this.activeTriangleIndex);
                if (this.circumcenterMesh) {
                    this.scene.add(this.circumcenterMesh.group);
                }
            } else {
                this.activeTriangleIndex = -1;
            }
        }
    }

    private regenerate() {
        if (this.currentMesh) {
            this.scene.remove(this.currentMesh.group);
            this.currentMesh.dispose();
            this.currentMesh = null;
        }
        if (this.currentCdtHandle) {
            this.currentCdtHandle.free();
            this.currentCdtHandle = null;
        }

        try {
            using _s = span('regenerate');

            const configJson = cdtParamsToJson(this.params);
            {
                using _s = span('generate_cdt');
                const cdt = generate_cdt(configJson);
                const error = cdt.error_message();
                if (error !== undefined) {
                    console.warn('CDT error:');
                    cdt.free();
                    return;
                }
                console.log(
                    `CDT: ${cdt.vertices().length / 2} vertices ` +
                        `${cdt.triangles().length / 3} triangles ` +
                        `${cdt.constraints().length / 2} constraints`
                );
                this.currentCdtHandle = cdt;
            }

            {
                using _s = span('buildCdtMesh');
                this.currentMesh = buildCdtMesh(this.currentCdtHandle!);
            }
            this.scene.add(this.currentMesh.group);

            this.updateCircumcenter();
        } catch (e) {
            console.error('CDT generation failed:', e);
        }
    }

    dispose() {
        window.removeEventListener('keydown', this.onKeyDown);

        if (this.currentMesh) {
            this.scene.remove(this.currentMesh.group);
            this.currentMesh.dispose();
        }
        if (this.circumcenterMesh) {
            this.scene.remove(this.circumcenterMesh.group);
            this.circumcenterMesh.dispose();
        }
        if (this.currentCdtHandle) {
            this.currentCdtHandle.free();
            this.currentCdtHandle = null;
        }

        super.dispose();
    }
}

export async function createCdtExperiment(container: HTMLElement, renderer: WebGPURenderer): Promise<CdtExperiment> {
    await init(wasmUrl);
    return new Cdt(container, renderer);
}
