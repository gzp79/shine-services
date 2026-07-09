import type { DebugPanel } from '../../engine/compositor/debug-panel';

export interface CdtParams {
    n_points: number;
    n_edges: number;
    seed: number;
}

export function defaultCdtParams(): CdtParams {
    return {
        n_points: 30,
        n_edges: 5,
        seed: 42
    };
}

export function cdtParamsToJson(p: CdtParams): string {
    return JSON.stringify({
        n_points: p.n_points,
        n_edges: p.n_edges,
        seed: p.seed,
        bound: 4096
    });
}

export function createCdtControls(debugPanel: DebugPanel, params: CdtParams, onChange: () => void): void {
    const folder = debugPanel.scope('Controls');
    folder.add(params, 'n_points', 3, 5000, 1).name('points').onChange(onChange);
    folder.add(params, 'n_edges', 0, 50, 1).name('constraints').onChange(onChange);
    const seedCtrl = folder.add(params, 'seed').name('seed').onChange(onChange);

    // "New Seed" button inserted inline after seed controller
    const seedRow = document.createElement('li');
    seedRow.style.cssText = 'display:flex;align-items:center;padding:0 var(--padding);height:var(--widget-height);';
    const label = document.createElement('span');
    label.style.cssText = 'flex:0 0 var(--name-width);min-width:var(--name-width);';
    const btn = document.createElement('button');
    btn.textContent = 'New Seed';
    btn.style.cssText =
        'flex:1;height:var(--widget-height);cursor:pointer;border:none;background:var(--widget-color);color:var(--text-color);font-family:inherit;font-size:inherit;border-radius:var(--widget-border-radius);';
    btn.addEventListener('mouseenter', () => (btn.style.background = 'var(--hover-color)'));
    btn.addEventListener('mouseleave', () => (btn.style.background = 'var(--widget-color)'));
    btn.addEventListener('click', () => {
        params.seed = Math.floor(Math.random() * 999999);
        seedCtrl.updateDisplay();
        onChange();
    });
    seedRow.appendChild(label);
    seedRow.appendChild(btn);
    seedCtrl.domElement.parentElement?.insertBefore(seedRow, seedCtrl.domElement.nextSibling);
}
