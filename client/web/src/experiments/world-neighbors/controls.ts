import GUI from 'lil-gui';

export type Params = {
    showHexagons: boolean;
    showAllInterior: boolean;
    showInterior: [boolean, boolean, boolean, boolean, boolean, boolean, boolean]; // 7 chunks
    showAllEdges: boolean;
    showEdges: [boolean, boolean, boolean, boolean, boolean, boolean]; // 6 edges
    showAllVertices: boolean;
    showVertices: [boolean, boolean, boolean, boolean, boolean, boolean]; // 6 vertices
};

export function defaultParams(): Params {
    return {
        showHexagons: true,
        showAllInterior: true,
        showInterior: [true, true, true, true, true, true, true],
        showAllEdges: true,
        showEdges: [true, true, true, true, true, true],
        showAllVertices: true,
        showVertices: [true, true, true, true, true, true]
    };
}

export function createControls(container: HTMLElement, params: Params, onDisplayChange: () => void): GUI {
    const gui = new GUI({ title: 'World Neighbors', container });
    gui.domElement.style.position = 'absolute';
    gui.domElement.style.top = '0';
    gui.domElement.style.right = '0';
    gui.domElement.style.zIndex = '10';

    gui.add(params, 'showHexagons').name('Show Hexagons').onChange(onDisplayChange);

    // Interior Cells - collapsible folder with master toggle inside
    const interiorFolder = gui.addFolder('Interior Cells (0=Center, 1-6=Neighbors)');
    interiorFolder.close(); // Collapsed by default

    interiorFolder
        .add(params, 'showAllInterior')
        .name('Show All')
        .onChange((value: boolean) => {
            params.showInterior.fill(value);
            onDisplayChange();
        });
    const interiorRow = document.createElement('div');
    interiorRow.style.cssText =
        'display:flex;gap:8px;padding:0 var(--padding);height:var(--widget-height);align-items:center;';
    for (let i = 0; i < 7; i++) {
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = params.showInterior[i];
        checkbox.style.cssText = 'cursor:pointer;';
        checkbox.addEventListener('change', () => {
            params.showInterior[i] = checkbox.checked;
            params.showAllInterior = params.showInterior.every((v) => v);
            onDisplayChange();
        });
        const label = document.createElement('label');
        label.textContent = String(i);
        label.style.cssText = 'display:flex;gap:4px;align-items:center;cursor:pointer;';
        label.prepend(checkbox);
        interiorRow.appendChild(label);
    }
    interiorFolder.$children.appendChild(interiorRow);

    // Boundary Edges - collapsible folder with master toggle inside
    const edgesFolder = gui.addFolder('Boundary Edges');
    edgesFolder.close(); // Collapsed by default

    edgesFolder
        .add(params, 'showAllEdges')
        .name('Show All')
        .onChange((value: boolean) => {
            params.showEdges.fill(value);
            onDisplayChange();
        });
    const edgesRow = document.createElement('div');
    edgesRow.style.cssText =
        'display:flex;gap:8px;padding:0 var(--padding);height:var(--widget-height);align-items:center;';
    for (let i = 0; i < 6; i++) {
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = params.showEdges[i];
        checkbox.style.cssText = 'cursor:pointer;';
        checkbox.addEventListener('change', () => {
            params.showEdges[i] = checkbox.checked;
            params.showAllEdges = params.showEdges.every((v) => v);
            onDisplayChange();
        });
        const label = document.createElement('label');
        label.textContent = String(i);
        label.style.cssText = 'display:flex;gap:4px;align-items:center;cursor:pointer;';
        label.prepend(checkbox);
        edgesRow.appendChild(label);
    }
    edgesFolder.$children.appendChild(edgesRow);

    // Boundary Vertices - collapsible folder with master toggle inside
    const verticesFolder = gui.addFolder('Boundary Vertices');
    verticesFolder.close(); // Collapsed by default

    verticesFolder
        .add(params, 'showAllVertices')
        .name('Show All')
        .onChange((value: boolean) => {
            params.showVertices.fill(value);
            onDisplayChange();
        });
    const verticesRow = document.createElement('div');
    verticesRow.style.cssText =
        'display:flex;gap:8px;padding:0 var(--padding);height:var(--widget-height);align-items:center;';
    for (let i = 0; i < 6; i++) {
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = params.showVertices[i];
        checkbox.style.cssText = 'cursor:pointer;';
        checkbox.addEventListener('change', () => {
            params.showVertices[i] = checkbox.checked;
            params.showAllVertices = params.showVertices.every((v) => v);
            onDisplayChange();
        });
        const label = document.createElement('label');
        label.textContent = String(i);
        label.style.cssText = 'display:flex;gap:4px;align-items:center;cursor:pointer;';
        label.prepend(checkbox);
        verticesRow.appendChild(label);
    }
    verticesFolder.$children.appendChild(verticesRow);

    return gui;
}
