import init, { generate_cdt } from '#wasm';
import wasmUrl from '#wasm-bin';
import { WebGPURenderer } from 'three/webgpu';
import { span } from '../../engine/utils';
import { ExperimentContext, animate, createExperiment } from '../experiment';
import { cdtParamsToJson, createCdtControls, defaultCdtParams } from './controls';
import { CdtMeshGroup, buildCdtMesh, buildCircumcenterMesh } from './mesh-builder';
import type { CdtMeshHandle } from '../../wasm-types/shine_game';

export interface CdtExperiment {
    dispose(): void;
}

export async function createCdtExperiment(container: HTMLElement, renderer: WebGPURenderer): Promise<CdtExperiment> {
    await init(wasmUrl);
    const ctx: ExperimentContext = createExperiment(container, renderer);

    // Adjust camera for the +-4096 coordinate range
    ctx.camera.near = 1;
    ctx.camera.far = 50000;
    ctx.camera.position.set(0, -7000, 12000);
    ctx.camera.lookAt(0, 0, 0);
    ctx.camera.updateProjectionMatrix();
    ctx.controls?.update();

    const params = defaultCdtParams();
    let currentCdtHandle: CdtMeshHandle | null = null;
    let currentMesh: CdtMeshGroup | null = null;
    let circumcenterMesh: CdtMeshGroup | null = null;
    let activeTriangleIndex = -1;
    let stopAnimation = () => {};

    function updateCircumcenter() {
        if (circumcenterMesh) {
            ctx.scene.remove(circumcenterMesh.group);
            circumcenterMesh.dispose();
            circumcenterMesh = null;
        }

        if (currentCdtHandle && activeTriangleIndex >= 0) {
            const triCount = currentCdtHandle.triangles().length / 3;
            if (activeTriangleIndex < triCount) {
                circumcenterMesh = buildCircumcenterMesh(currentCdtHandle, activeTriangleIndex);
                if (circumcenterMesh) {
                    ctx.scene.add(circumcenterMesh.group);
                }
            } else {
                activeTriangleIndex = -1;
            }
        }
    }

    function regenerate() {
        if (currentMesh) {
            ctx.scene.remove(currentMesh.group);
            currentMesh.dispose();
            currentMesh = null;
        }
        if (currentCdtHandle) {
            currentCdtHandle.free();
            currentCdtHandle = null;
        }

        try {
            using _s = span('regenerate');

            const configJson = cdtParamsToJson(params);
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
                currentCdtHandle = cdt;
            }

            {
                using _s = span('buildCdtMesh');
                currentMesh = buildCdtMesh(currentCdtHandle!);
            }
            ctx.scene.add(currentMesh.group);

            updateCircumcenter();
        } catch (e) {
            console.error('CDT generation failed:', e);
        }
    }

    const onKeyDown = (e: KeyboardEvent) => {
        if (e.key === '+' || e.key === '=') {
            activeTriangleIndex++;
            updateCircumcenter();
        } else if (e.key === '-' || e.key === '_') {
            if (activeTriangleIndex >= 0) {
                activeTriangleIndex--;
                updateCircumcenter();
            }
        }
    };

    window.addEventListener('keydown', onKeyDown);

    const gui = createCdtControls(container, params, regenerate);
    regenerate();
    stopAnimation = animate(ctx);

    return {
        dispose() {
            window.removeEventListener('keydown', onKeyDown);
            stopAnimation();
            gui.destroy();

            if (currentMesh) {
                ctx.scene.remove(currentMesh.group);
                currentMesh.dispose();
            }
            if (circumcenterMesh) {
                ctx.scene.remove(circumcenterMesh.group);
                circumcenterMesh.dispose();
            }
            if (currentCdtHandle) {
                currentCdtHandle.free();
                currentCdtHandle = null;
            }

            ctx.resizeObserver.disconnect();
        }
    };
}
