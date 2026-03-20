import GUI from 'lil-gui';

export interface MeshParams {
    subdivision: number;
    orientation: string;
    smoothing: string;
    seed: number;
    // Lloyd
    lloyd_iterations: number;
    lloyd_strength: number;
    lloyd_weight_min: number;
    lloyd_weight_max: number;
    // Noise
    noise_amplitude: number;
    noise_frequency: number;
    // Cotangent
    cotangent_iterations: number;
    cotangent_strength: number;
    // Spring
    spring_iterations: number;
    spring_dt: number;
    spring_spring_strength: number;
    spring_shape_strength: number;
    // Jitter
    jitter_amplitude: number;
    // Fix quads
    fix_enabled: boolean;
    fix_min_quality: number;
    fix_max_iterations: number;
}

export function defaultParams(): MeshParams {
    return {
        subdivision: 3,
        orientation: 'Even',
        smoothing: 'None',
        seed: 42,
        lloyd_iterations: 20,
        lloyd_strength: 0.4,
        lloyd_weight_min: 2.5,
        lloyd_weight_max: 15.5,
        noise_amplitude: 0.5,
        noise_frequency: 5.0,
        cotangent_iterations: 10,
        cotangent_strength: 0.5,
        spring_iterations: 50,
        spring_dt: 0.1,
        spring_spring_strength: 0.3,
        spring_shape_strength: 0.5,
        jitter_amplitude: 2.0,
        fix_enabled: true,
        fix_min_quality: 0.15,
        fix_max_iterations: 50
    };
}

export function paramsToConfigJson(p: MeshParams): string {
    const smoothing: Record<string, unknown> = { method: p.smoothing };

    switch (p.smoothing) {
        case 'Lloyd':
            smoothing.iterations = p.lloyd_iterations;
            smoothing.strength = p.lloyd_strength;
            smoothing.weight_min = p.lloyd_weight_min;
            smoothing.weight_max = p.lloyd_weight_max;
            break;
        case 'Noise':
            smoothing.amplitude = p.noise_amplitude;
            smoothing.frequency = p.noise_frequency;
            break;
        case 'Cotangent':
            smoothing.iterations = p.cotangent_iterations;
            smoothing.strength = p.cotangent_strength;
            break;
        case 'Spring':
            smoothing.iterations = p.spring_iterations;
            smoothing.dt = p.spring_dt;
            smoothing.spring_strength = p.spring_spring_strength;
            smoothing.shape_strength = p.spring_shape_strength;
            break;
        case 'Jitter':
            smoothing.amplitude = p.jitter_amplitude;
            break;
    }

    return JSON.stringify({
        subdivision: p.subdivision,
        orientation: p.orientation,
        seed: p.seed,
        smoothing,
        fix_quads: {
            enabled: p.fix_enabled,
            min_quality: p.fix_min_quality,
            max_iterations: p.fix_max_iterations
        }
    });
}

export function createControls(container: HTMLElement, params: MeshParams, onChange: () => void): GUI {
    const gui = new GUI({ title: 'Hex Mesh', container });
    gui.domElement.style.position = 'absolute';
    gui.domElement.style.top = '0';
    gui.domElement.style.right = '0';
    gui.domElement.style.zIndex = '10';

    gui.add(params, 'subdivision', 0, 5, 1).onChange(onChange);
    const orientObj = { odd: params.orientation === 'Odd' };
    gui.add(orientObj, 'odd')
        .name('odd orientation')
        .onChange((v: boolean) => {
            params.orientation = v ? 'Odd' : 'Even';
            onChange();
        });
    gui.add(params, 'smoothing', ['None', 'Lloyd', 'Noise', 'Cotangent', 'Spring', 'Jitter']).onChange(() => {
        rebuildAdvanced();
        onChange();
    });
    const seedCtrl = gui.add(params, 'seed').name('seed').onChange(onChange);
    // Add "New Seed" as a second row with the button only in the widget column
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

    let advancedFolder: GUI | null = null;

    function rebuildAdvanced() {
        if (advancedFolder) {
            advancedFolder.destroy();
            advancedFolder = null;
        }

        if (params.smoothing === 'None') return;

        advancedFolder = gui.addFolder('Advanced');
        advancedFolder.close();

        switch (params.smoothing) {
            case 'Lloyd':
                advancedFolder.add(params, 'lloyd_iterations', 1, 50, 1).name('iterations').onChange(onChange);
                advancedFolder.add(params, 'lloyd_strength', 0, 1, 0.01).name('strength').onChange(onChange);
                advancedFolder.add(params, 'lloyd_weight_min', 0.001, 20, 0.1).name('weight min').onChange(onChange);
                advancedFolder.add(params, 'lloyd_weight_max', 0.001, 30, 0.1).name('weight max').onChange(onChange);
                break;
            case 'Noise':
                advancedFolder.add(params, 'noise_amplitude', 0, 2, 0.01).name('amplitude').onChange(onChange);
                advancedFolder.add(params, 'noise_frequency', 0.5, 20, 0.1).name('frequency').onChange(onChange);
                break;
            case 'Cotangent':
                advancedFolder.add(params, 'cotangent_iterations', 1, 50, 1).name('iterations').onChange(onChange);
                advancedFolder.add(params, 'cotangent_strength', 0, 1, 0.01).name('strength').onChange(onChange);
                break;
            case 'Spring':
                advancedFolder.add(params, 'spring_iterations', 1, 200, 1).name('iterations').onChange(onChange);
                advancedFolder.add(params, 'spring_dt', 0.01, 0.5, 0.01).name('dt').onChange(onChange);
                advancedFolder
                    .add(params, 'spring_spring_strength', 0, 2, 0.01)
                    .name('spring strength')
                    .onChange(onChange);
                advancedFolder
                    .add(params, 'spring_shape_strength', 0, 2, 0.01)
                    .name('shape strength')
                    .onChange(onChange);
                break;
            case 'Jitter':
                advancedFolder.add(params, 'jitter_amplitude', 0, 5, 0.01).name('amplitude').onChange(onChange);
                break;
        }
    }

    // Fix quads folder — always visible (useful even without smoothing)
    const fixFolder = gui.addFolder('Fix Quads');
    fixFolder.add(params, 'fix_enabled').name('enabled').onChange(onChange);
    fixFolder.add(params, 'fix_min_quality', 0.01, 0.5, 0.01).name('min quality').onChange(onChange);
    fixFolder.add(params, 'fix_max_iterations', 1, 200, 1).name('max iterations').onChange(onChange);
    fixFolder.close();

    // Build initial advanced section
    rebuildAdvanced();

    return gui;
}
