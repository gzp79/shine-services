import { createCdtViewer } from './cdt-index';
import { createHexMeshViewer } from './index';

const container = document.getElementById('app')!;

type Viewer = { destroy(): void };
let current: Viewer | null = null;

async function route() {
    if (current) {
        current.destroy();
        current = null;
    }

    const hash = window.location.hash;
    if (hash === '#cdt') {
        current = await createCdtViewer(container);
    } else {
        current = await createHexMeshViewer(container);
    }
}

window.addEventListener('hashchange', () => void route());
void route();
