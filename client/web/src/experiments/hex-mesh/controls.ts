import GUI from 'lil-gui';

const filtersList = ['None', 'Laplacian', 'Jitter', 'QuadRelax', 'VertexRepulsion'] as const;
type FilterType = (typeof filtersList)[number];

export type FilterEntryBase = {
    type: FilterType;
    enabled: boolean;
};
export type FilterEntry = FilterEntryBase &
    (
        | {
              type: 'None';
          }
        | {
              type: 'Laplacian';
              strength: number;
              iterations: number;
          }
        | {
              type: 'Jitter';
              amplitude: number;
          }
        | {
              type: 'QuadRelax';
              quality: number;
              strength: number;
              iterations: number;
          }
        | {
              type: 'VertexRepulsion';
              strength: number;
              iterations: number;
          }
    );

function defaultFilter(type: FilterType): FilterEntry {
    switch (type) {
        case 'None':
            return { type, enabled: false };
        case 'Laplacian':
            return { type, enabled: true, strength: 0.5, iterations: 50 };
        case 'Jitter':
            return { type, enabled: true, amplitude: 1 };
        case 'QuadRelax':
            return { type, enabled: true, quality: 0.25, strength: 0.5, iterations: 50 };
        case 'VertexRepulsion':
            return { type, enabled: true, strength: 1.0, iterations: 100 };
        default:
            return { type: 'None', enabled: false };
    }
}

const meshersList = ['Patch', 'Cdt', 'Lattice'] as const;
type MesherType = (typeof meshersList)[number];

export type MesherEntryBase = {
    type: MesherType;
    subdivision: number;
};

export type PatchOrientation = 'Odd' | 'Even';

export type MesherEntry = MesherEntryBase &
    (
        | {
              type: 'Patch';
              orientation: PatchOrientation;
          }
        | {
              type: 'Cdt';
              interior_points: number;
          }
        | {
              type: 'Lattice';
          }
    );

function defaultMesher(type: MesherType, prev?: MesherEntry): MesherEntry {
    const subdivision = prev?.subdivision ?? 3;
    switch (type) {
        case 'Lattice':
            return { type, subdivision };
        case 'Cdt':
            return { type, subdivision, interior_points: 50 };
        case 'Patch':
            return { type, subdivision, orientation: 'Odd' };
        default:
            return { type, subdivision };
    }
}

export type Params = {
    showPrimal: boolean;
    showDual: boolean;
    seed: number;
    mesher: MesherEntry;
    filters: FilterEntry[];
};

export function defaultParams(): Params {
    return {
        showDual: false,
        showPrimal: true,
        seed: 42,
        mesher: defaultMesher('Lattice'),
        filters: []
    };
}

export function paramsToConfigJson(params: Params): string {
    const filters = params.filters.filter((f) => f.type !== 'None' && f.enabled);

    return JSON.stringify({
        ...params,
        filters
    });
}

export function createControls(
    container: HTMLElement,
    params: Params,
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
    globalFolder.add(params, 'showPrimal').name('primal wireframe').onChange(onDisplayChange);
    globalFolder.add(params, 'showDual').name('dual wireframe').onChange(onDisplayChange);

    const seedCtrl = globalFolder.add(params, 'seed').name('seed').onChange(onChange);
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

    // Mesher
    const mesherFolder = gui.addFolder('Mesher');
    mesherFolder
        .add(params.mesher, 'type', meshersList)
        .name('type')
        .onChange((v: string) => {
            params.mesher = defaultMesher(v as MesherType, params.mesher);
            rebuildMesherParams();
            onChange();
        });

    let mesherCtrls: GUI['controllers'][number][] = [];

    function rebuildMesherParams() {
        for (const c of mesherCtrls) c.destroy();
        mesherCtrls = [];

        const mesher = params.mesher;
        mesherCtrls.push(mesherFolder.add(mesher, 'subdivision', 1, 5, 1).name('subdivision').onChange(onChange));
        if (mesher.type === 'Patch') {
            const orientObj = { odd: mesher.orientation === 'Odd' };
            mesherCtrls.push(
                mesherFolder
                    .add(orientObj, 'odd')
                    .name('odd orientation')
                    .onChange((v: boolean) => {
                        mesher.orientation = v ? 'Odd' : 'Even';
                        onChange();
                    })
            );
        } else if (mesher.type === 'Cdt') {
            mesherCtrls.push(
                mesherFolder.add(mesher, 'interior_points', 0, 2000, 1).name('interior points').onChange(onChange)
            );
        } else if (mesher.type === 'Lattice') {
            // nop
        }
    }

    // Filters
    let filterFolders: GUI[] = [];

    function rebuildFilters() {
        for (const f of filterFolders) f.destroy();
        filterFolders = [];

        params.filters.forEach((entry, idx) => {
            const folder = gui.addFolder(entry.type);
            filterFolders.push(folder);

            const typeCtrl = folder
                .add(entry, 'type', filtersList)
                .name('type')
                .onChange((v: string) => {
                    if (v === 'None') {
                        params.filters.splice(idx, 1);
                    } else {
                        params.filters[idx] = defaultFilter(v as FilterType);
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
                    folder.add(entry, 'strength', 0, 1, 0.01).name('strength').onChange(onChange);
                    folder.add(entry, 'iterations', 1, 200, 1).name('iterations').onChange(onChange);
                    break;
                case 'Jitter':
                    folder.add(entry, 'amplitude', 0, 5, 0.01).name('amplitude').onChange(onChange);
                    break;
                case 'QuadRelax':
                    folder.add(entry, 'quality', 0, 1, 0.01).name('min quality').onChange(onChange);
                    folder.add(entry, 'strength', 0, 1, 0.01).name('strength').onChange(onChange);
                    folder.add(entry, 'iterations', 1, 200, 1).name('max iterations').onChange(onChange);
                    break;
                case 'VertexRepulsion':
                    folder.add(entry, 'strength', 0, 1, 0.01).name('strength').onChange(onChange);
                    folder.add(entry, 'iterations', 1, 200, 1).name('iterations').onChange(onChange);
                    break;
            }
        });

        // Trailing "None" dropdown to add a new filter
        const addObj = { type: 'None' };
        const addFolder = gui.addFolder('Add');
        filterFolders.push(addFolder);
        addFolder
            .add(addObj, 'type', filtersList)
            .name('type')
            .onChange((v: string) => {
                if (v !== 'None') {
                    params.filters.push(defaultFilter(v as FilterType));
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
