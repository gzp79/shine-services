import GUI from 'lil-gui';

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

export function createCdtControls(container: HTMLElement, params: CdtParams, onChange: () => void): GUI {
    const gui = new GUI({ title: 'CDT', container });
    gui.domElement.style.position = 'absolute';
    gui.domElement.style.top = '0';
    gui.domElement.style.right = '0';
    gui.domElement.style.zIndex = '10';

    gui.add(params, 'n_points', 3, 5000, 1).name('points').onChange(onChange);
    gui.add(params, 'n_edges', 0, 50, 1).name('constraints').onChange(onChange);
    const seedCtrl = gui.add(params, 'seed').name('seed').onChange(onChange);

    // "New Seed" button
    const seedRow = document.createElement('li');
    seedRow.style.cssText = 'display:flex;align-items:center;padding:0 var(--padding);height:var(--widget-height);';
    const label = document.createElement('span');
    label.textContent = '';
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

    return gui;
}
