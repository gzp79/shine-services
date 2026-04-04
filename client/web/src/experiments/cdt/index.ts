import init, { generate_cdt } from '#wasm';
import wasmUrl from '#wasm-bin';
import { ExperimentContext, animate, createExperiment } from '../experiment';
import { cdtParamsToJson, createCdtControls, defaultCdtParams } from './controls';
import { CdtMeshGroup, buildCdtMesh } from './mesh-builder';

export interface CdtViewer {
    dispose(): void;
}

export async function createCdtViewer(container: HTMLElement): Promise<CdtViewer> {
    await init(wasmUrl);

    const ctx: ExperimentContext = createExperiment(container);
    // Adjust camera for the +-4096 coordinate range
    ctx.camera.near = 1;
    ctx.camera.far = 50000;
    ctx.camera.position.set(0, -7000, 12000);
    ctx.camera.lookAt(0, 0, 0);
    ctx.camera.updateProjectionMatrix();
    ctx.controls.update();

    const params = defaultCdtParams();
    let currentMesh: CdtMeshGroup | null = null;
    let animationId = 0;

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

            const data = {
                vertices: new Float32Array(wasmCdt.vertices()),
                triangles: new Uint32Array(wasmCdt.triangles()),
                fixedEdges: new Uint32Array(wasmCdt.fixed_edges())
            };

            console.log(`CDT: ${wasmCdt.vertex_count()} vertices, ${wasmCdt.triangle_count()} triangles`);
            wasmCdt.free();

            currentMesh = buildCdtMesh(data);
            ctx.scene.add(currentMesh.group);
        } catch (e) {
            console.error('CDT generation failed:', e);
        }
    }

    const gui = createCdtControls(container, params, regenerate);
    regenerate();
    animationId = animate(ctx);

    return {
        dispose() {
            cancelAnimationFrame(animationId);
            gui.destroy();
            if (currentMesh) {
                ctx.scene.remove(currentMesh.group);
                currentMesh.dispose();
            }
            ctx.resizeObserver.disconnect();
            ctx.renderer.dispose();
            ctx.renderer.domElement.remove();
        }
    };
}
