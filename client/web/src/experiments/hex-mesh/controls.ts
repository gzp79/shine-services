import GUI from 'lil-gui';

export interface GlobalParams {
    showPrimal: boolean;
    showDual: boolean;
    seed: number;
}

export interface FilterEntry {
    type: string;
    enabled: boolean;
    // Laplacian
    iterations: number;
    strength: number;
    // Jitter
    amplitude: number;
    // QuadRelax
    min_quality: number;
    relax_strength: number;
    max_iterations: number;
    // EnergyRelax
    area_weight: number;
    shape_weight: number;
    step_size: number;
    energy_iterations: number;
    // VertexRepulsion
    repulsion_strength: number;
    repulsion_iterations: number;
}

export interface MeshParams {
    mesher: string;
    // Patch
    subdivision: number;
    orientation: string;
    // Cdt
    edge_subdivisions: number;
    interior_points: number;
    // Lattice
    lattice_subdivision: number;
    // Filters
    filters: FilterEntry[];
}

function defaultFilter(type: string): FilterEntry {
    return {
        type,
        enabled: true,
        iterations: 20,
        strength: 0.5,
        amplitude: 0.3,
        min_quality: 0.15,
        relax_strength: 0.5,
        max_iterations: 50,
        area_weight: 1.0,
        shape_weight: 1.0,
        step_size: 0.01,
        energy_iterations: 50,
        repulsion_strength: 0.2,
        repulsion_iterations: 10,
    };
}

export function defaultGlobalParams(): GlobalParams {
    return {
        showPrimal: true,
        showDual: false,
        seed: 42
    };
}

export function defaultParams(): MeshParams {
    return {
        mesher: 'Patch',
        subdivision: 3,
        orientation: 'Even',
        edge_subdivisions: 4,
        interior_points: 20,
        lattice_subdivision: 3,
        filters: []
    };
}

export function paramsToConfigJson(p: MeshParams, g: GlobalParams): string {
    let mesher: Record<string, unknown>;
    if (p.mesher === 'Cdt') {
        mesher = { type: 'Cdt', edge_subdivisions: p.edge_subdivisions, interior_points: p.interior_points };
    } else if (p.mesher === 'Lattice') {
        mesher = { type: 'Lattice', subdivision: p.lattice_subdivision };
    } else {
        mesher = { type: 'Patch', subdivision: p.subdivision, orientation: p.orientation };
    }

    const filters = p.filters
        .filter((f) => f.type !== 'None' && f.enabled)
        .map((f) => {
            switch (f.type) {
                case 'Laplacian':
                    return { type: 'Laplacian', iterations: f.iterations, strength: f.strength };
                case 'Jitter':
                    return { type: 'Jitter', amplitude: f.amplitude };
                case 'QuadRelax':
                    return { type: 'QuadRelax', min_quality: f.min_quality, strength: f.relax_strength, max_iterations: f.max_iterations };
                case 'EnergyRelax':
                    return { type: 'EnergyRelax', area_weight: f.area_weight, shape_weight: f.shape_weight, step_size: f.step_size, iterations: f.energy_iterations };
                case 'VertexRepulsion':
                    return { type: 'VertexRepulsion', strength: f.repulsion_strength, iterations: f.repulsion_iterations };
                default:
                    return { type: f.type };
            }
        });

    return JSON.stringify({
        mesher,
        seed: g.seed,
        filters
    });
}

const FILTER_TYPES = ['None', 'Laplacian', 'Jitter', 'QuadRelax', 'EnergyRelax', 'VertexRepulsion'];

export function createControls(
    container: HTMLElement,
    params: MeshParams,
    globalParams: GlobalParams,
    onChange: () => void,
    onDisplayChange: () => void
): GUI {
    const gui = new GUI({ title: 'Hex Mesh', container });
    gui.domElement.style.position = 'absolute';
    gui.domElement.style.top = '0';
    gui.domElement.style.right = '0';
    gui.domElement.style.zIndex = '10';

    // ── Global ────────────────────────────────────────────────────────
    const globalFolder = gui.addFolder('Global');
    globalFolder.add(globalParams, 'showPrimal').name('primal wireframe').onChange(onDisplayChange);
    globalFolder.add(globalParams, 'showDual').name('dual wireframe').onChange(onDisplayChange);

    const seedCtrl = globalFolder.add(globalParams, 'seed').name('seed').onChange(onChange);
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
        globalParams.seed = Math.floor(Math.random() * 999999);
        seedCtrl.updateDisplay();
        onChange();
    });
    seedRow.appendChild(label);
    seedRow.appendChild(btn);
    seedCtrl.domElement.parentElement?.insertBefore(seedRow, seedCtrl.domElement.nextSibling);

    // ── Mesher ────────────────────────────────────────────────────────
    const mesherFolder = gui.addFolder('Mesher');
    mesherFolder.add(params, 'mesher', ['Patch', 'Cdt', 'Lattice']).name('type').onChange(() => {
        rebuildMesherParams();
        onChange();
    });

    let mesherCtrls: GUI['controllers'][number][] = [];

    function rebuildMesherParams() {
        for (const c of mesherCtrls) c.destroy();
        mesherCtrls = [];

        if (params.mesher === 'Patch') {
            mesherCtrls.push(mesherFolder.add(params, 'subdivision', 0, 5, 1).onChange(onChange));
            const orientObj = { odd: params.orientation === 'Odd' };
            mesherCtrls.push(
                mesherFolder
                    .add(orientObj, 'odd')
                    .name('odd orientation')
                    .onChange((v: boolean) => {
                        params.orientation = v ? 'Odd' : 'Even';
                        onChange();
                    })
            );
        } else if (params.mesher === 'Cdt') {
            mesherCtrls.push(mesherFolder.add(params, 'edge_subdivisions', 1, 5, 1).name('edge subdivisions').onChange(onChange));
            mesherCtrls.push(mesherFolder.add(params, 'interior_points', 0, 500, 1).name('interior points').onChange(onChange));
        } else if (params.mesher === 'Lattice') {
            mesherCtrls.push(mesherFolder.add(params, 'lattice_subdivision', 0, 5, 1).name('subdivision').onChange(onChange));
        }
    }

    // ── Filters ──────────────────────────────────────────────────────
    // Each filter is a top-level folder. A trailing "None" dropdown lets users
    // append new filters; switching an existing filter to "None" removes it.

    let filterFolders: GUI[] = [];

    function rebuildFilters() {
        for (const f of filterFolders) f.destroy();
        filterFolders = [];

        params.filters.forEach((entry, idx) => {
            const folder = gui.addFolder(entry.type);
            filterFolders.push(folder);

            const typeCtrl = folder.add(entry, 'type', FILTER_TYPES).name('type').onChange((v: string) => {
                if (v === 'None') {
                    params.filters.splice(idx, 1);
                } else {
                    params.filters[idx] = defaultFilter(v);
                }
                rebuildFilters();
                onChange();
            });

            // Inline enabled checkbox next to the type dropdown
            const cb = document.createElement('input');
            cb.type = 'checkbox';
            cb.checked = entry.enabled;
            cb.style.cssText = 'width:16px;height:16px;margin:0 4px;cursor:pointer;flex-shrink:0;';
            cb.addEventListener('pointerdown', (e) => e.stopPropagation());
            cb.addEventListener('click', (e) => e.stopPropagation());
            cb.addEventListener('change', () => {
                entry.enabled = cb.checked;
                onChange();
            });
            const widget = typeCtrl.domElement.querySelector('.widget');
            if (widget) {
                typeCtrl.domElement.insertBefore(cb, widget);
            }

            switch (entry.type) {
                case 'Laplacian':
                    folder.add(entry, 'iterations', 1, 50, 1).name('iterations').onChange(onChange);
                    folder.add(entry, 'strength', 0, 1, 0.01).name('strength').onChange(onChange);
                    break;
                case 'Jitter':
                    folder.add(entry, 'amplitude', 0, 5, 0.01).name('amplitude').onChange(onChange);
                    break;
                case 'QuadRelax':
                    folder.add(entry, 'min_quality', 0, 1, 0.01).name('min quality').onChange(onChange);
                    folder.add(entry, 'relax_strength', 0, 1, 0.01).name('strength').onChange(onChange);
                    folder.add(entry, 'max_iterations', 1, 200, 1).name('max iterations').onChange(onChange);
                    break;
                case 'EnergyRelax':
                    folder.add(entry, 'area_weight', 0, 5, 0.01).name('area weight').onChange(onChange);
                    folder.add(entry, 'shape_weight', 0, 5, 0.01).name('shape weight').onChange(onChange);
                    folder.add(entry, 'step_size', 0.001, 0.1, 0.001).name('step size').onChange(onChange);
                    folder.add(entry, 'energy_iterations', 1, 200, 1).name('iterations').onChange(onChange);
                    break;
                case 'VertexRepulsion':
                    folder.add(entry, 'repulsion_strength', 0.01, 0.5, 0.01).name('strength').onChange(onChange);
                    folder.add(entry, 'repulsion_iterations', 1, 100, 1).name('iterations').onChange(onChange);
                    break;
            }
        });

        // Trailing "None" dropdown to add a new filter
        const addObj = { type: 'None' };
        const addFolder = gui.addFolder('Add');
        filterFolders.push(addFolder);
        addFolder.add(addObj, 'type', FILTER_TYPES).name('type').onChange((v: string) => {
            if (v !== 'None') {
                params.filters.push(defaultFilter(v));
                rebuildFilters();
                onChange();
            }
        });
    }

    // Build initial dynamic sections
    rebuildMesherParams();
    rebuildFilters();

    return gui;
}
