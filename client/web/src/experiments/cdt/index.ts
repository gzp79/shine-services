import init, { generate_cdt } from '#wasm';
import wasmUrl from '#wasm-bin';
import { ExperimentContext, animate, createExperiment } from '../experiment';
import { cdtParamsToJson, createCdtControls, defaultCdtParams } from './controls';
import { CdtData, CdtMeshGroup, buildCdtMesh, buildCircumcenterMesh } from './mesh-builder';

export interface CdtExperiment {
    dispose(): void;
}

export async function createCdtExperiment(container: HTMLElement): Promise<CdtExperiment> {
    await init(wasmUrl);
    const ctx: ExperimentContext = await createExperiment(container);

    // Adjust camera for the +-4096 coordinate range
    ctx.camera.near = 1;
    ctx.camera.far = 50000;
    ctx.camera.position.set(0, -7000, 12000);
    ctx.camera.lookAt(0, 0, 0);
    ctx.camera.updateProjectionMatrix();
    ctx.controls?.update();

    const params = defaultCdtParams();
    let currentData: CdtData | null = null;
    let currentMesh: CdtMeshGroup | null = null;
    let circumcenterMesh: CdtMeshGroup | null = null;
    let activeTriangleIndex = -1;
    let animationId = 0;

    function updateCircumcenter() {
        if (circumcenterMesh) {
            ctx.scene.remove(circumcenterMesh.group);
            circumcenterMesh.dispose();
            circumcenterMesh = null;
        }

        if (currentData && activeTriangleIndex >= 0) {
            const triCount = currentData.triangles.length / 3;
            if (activeTriangleIndex < triCount) {
                circumcenterMesh = buildCircumcenterMesh(currentData, activeTriangleIndex);
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

        try {
            const configJson = cdtParamsToJson(params);
            const wasmCdt = generate_cdt(configJson);

            if (wasmCdt.has_error()) {
                console.warn('CDT error:', wasmCdt.error_message());
                wasmCdt.free();
                return;
            }

            currentData = {
                vertices: new Float32Array(wasmCdt.vertices()),
                triangles: new Uint32Array(wasmCdt.triangles()),
                fixedEdges: new Uint32Array(wasmCdt.fixed_edges())
            };

            console.log(`CDT: ${wasmCdt.vertex_count()} vertices, ${wasmCdt.triangle_count()} triangles`);
            wasmCdt.free();

            currentMesh = buildCdtMesh(currentData);
            ctx.scene.add(currentMesh.group);

            // Re-sync circumcenter if visible
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
    animationId = animate(ctx);

    return {
        dispose() {
            window.removeEventListener('keydown', onKeyDown);
            cancelAnimationFrame(animationId);
            gui.destroy();

            if (currentMesh) {
                ctx.scene.remove(currentMesh.group);
                currentMesh.dispose();
            }
            if (circumcenterMesh) {
                ctx.scene.remove(circumcenterMesh.group);
                circumcenterMesh.dispose();
            }

            ctx.resizeObserver.disconnect();
            ctx.renderer.dispose();
            ctx.renderer.domElement.remove();
        }
    };
}
