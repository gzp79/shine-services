const container = document.getElementById('app')!;

type Viewer = { dispose(): void };
let current: Viewer | null = null;

async function route() {
    if (current) {
        current.dispose();
        current = null;
    }

    const hash = window.location.hash;
    if (hash === '#hex-mesh') {
        const { createHexMeshViewer } = await import('./experiments/hex-mesh/index');
        current = await createHexMeshViewer(container);
    } else if (hash === '#cdt') {
        const { createCdtViewer } = await import('./experiments/cdt/index');
        current = await createCdtViewer(container);
    } else if (hash === '#input-events') {
        const { createInputEventsViewer } = await import('./experiments/input-events/index');
        current = await createInputEventsViewer(container);
    } else {
        const { createGame } = await import('./engine/game');
        current = await createGame(container);
    }
}

window.addEventListener('hashchange', () => void route());
void route();
