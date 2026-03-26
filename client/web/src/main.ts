const container = document.getElementById('app')!;

type Viewer = { destroy(): void };
let current: Viewer | null = null;

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
        const { createGame } = await import('./game');
        current = await createGame(container);
    }
}

window.addEventListener('hashchange', () => void route());
void route();
