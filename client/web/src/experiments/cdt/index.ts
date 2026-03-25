import init, { generate_cdt } from '#wasm';
import wasmUrl from '#wasm-bin';
import { SceneContext, animate, createScene } from '../../scene';
import { cdtParamsToJson, createCdtControls, defaultCdtParams } from './controls';
import { CdtMeshGroup, buildCdtMesh } from './mesh-builder';

export interface CdtViewer {
    destroy(): void;
}

export async function createCdtViewer(container: HTMLElement): Promise<CdtViewer> {
    await init(wasmUrl);

    const ctx: SceneContext = createScene(container);
    // Adjust camera for the +-4096 coordinate range (default far=1000 is too small)
    ctx.camera.near = 1;
    ctx.camera.far = 50000;
    ctx.camera.position.set(0, 12000, 7000);
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
        destroy() {
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
