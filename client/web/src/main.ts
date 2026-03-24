import { SceneContext, animate, createScene } from './scene';

const container = document.getElementById('app')!;

type Viewer = { destroy(): void };
let current: Viewer | null = null;

function defaultViewer(): Viewer {
    const ctx: SceneContext = createScene(container);
    const animationId = animate(ctx);

    return {
        destroy() {
            cancelAnimationFrame(animationId);
            ctx.resizeObserver.disconnect();
            ctx.renderer.dispose();
            ctx.renderer.domElement.remove();
        }
    };
}

async function route() {
    if (current) {
        current.destroy();
        current = null;
    }

    const hash = window.location.hash;
    if (hash === '#hex-mesh') {
        const { createHexMeshViewer } = await import('./experiments/hex-mesh/index');
        current = await createHexMeshViewer(container);
    } else if (hash === '#cdt') {
        const { createCdtViewer } = await import('./experiments/cdt/index');
        current = await createCdtViewer(container);
    } else {
        current = defaultViewer();
    }
}

window.addEventListener('hashchange', () => void route());
void route();
