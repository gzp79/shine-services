import * as THREE from 'three';
import { DragControls } from 'three/addons/controls/DragControls.js';

export interface ControlBoxOptions {
    scene: THREE.Scene;
    camera: THREE.Camera;
    domElement: HTMLElement;
    onChanged?: () => void;
    onDragStart?: () => void;
    onDragEnd?: () => void;
}

export class ControlBox {
    private readonly scene: THREE.Scene;
    private dragControls: DragControls;
    private wireframe: THREE.LineSegments | null = null;

    private readonly onChanged?: () => void;
    private readonly onDragStartCallback?: () => void;
    private readonly onDragEndCallback?: () => void;

    private readonly controlPoints: THREE.Mesh[] = [];
    private readonly cornerPositions: THREE.Vector3[] = [];

    constructor(options: ControlBoxOptions) {
        this.scene = options.scene;
        this.onChanged = options.onChanged;
        this.onDragStartCallback = options.onDragStart;
        this.onDragEndCallback = options.onDragEnd;

        // Initialize control points at [-1,1]^3 corners
        this.initializeControlPoints();

        // Setup drag controls
        this.dragControls = new DragControls(this.controlPoints, options.camera, options.domElement);
        this.dragControls.addEventListener('dragstart', this.onDragStart);
        this.dragControls.addEventListener('drag', this.onDrag);
        this.dragControls.addEventListener('dragend', this.onDragEnd);

        // Create initial wireframe
        this.updateWireframe();
    }

    get corners(): THREE.Vector3[] {
        return this.cornerPositions;
    }

    set corners(positions: THREE.Vector3[]) {
        if (positions.length !== 8) {
            throw new Error('ControlBox requires exactly 8 corner positions');
        }

        // Update control point positions
        for (let i = 0; i < 8; i++) {
            this.controlPoints[i].position.copy(positions[i]);
            this.cornerPositions[i].copy(positions[i]);
        }

        this.updateWireframe();
        this.onChanged?.();
    }

    private initializeControlPoints(): void {
        const positions = [
            [-1, -1, -1], // 0: bottom-far-left
            [1, -1, -1], // 1: bottom-far-right
            [-1, 1, -1], // 2: top-far-left
            [1, 1, -1], // 3: top-far-right
            [-1, -1, 1], // 4: bottom-near-left
            [1, -1, 1], // 5: bottom-near-right
            [-1, 1, 1], // 6: top-near-left
            [1, 1, 1] // 7: top-near-right
        ];

        const colors = ['#000000', '#ff0000', '#00ff00', '#ffff00', '#0000ff', '#ff00ff', '#00ffff', '#ffffff'];

        positions.forEach(([x, y, z], i) => {
            const geometry = new THREE.SphereGeometry(0.1, 16, 16);
            const material = new THREE.MeshBasicMaterial({
                color: colors[i],
                transparent: true,
                opacity: 0.8
            });
            const mesh = new THREE.Mesh(geometry, material);
            mesh.position.set(x, y, z);

            this.controlPoints.push(mesh);
            this.cornerPositions.push(mesh.position);
            this.scene.add(mesh);
        });
    }

    private updateWireframe(): void {
        // Remove old wireframe
        if (this.wireframe) {
            this.scene.remove(this.wireframe);
            this.wireframe.geometry.dispose();
            (this.wireframe.material as THREE.Material).dispose();
        }

        if (this.cornerPositions.length !== 8) return;

        // Create edges connecting the 8 control points
        const positions = new Float32Array([
            // Bottom face edges
            ...this.cornerPositions[0].toArray(),
            ...this.cornerPositions[1].toArray(),
            ...this.cornerPositions[1].toArray(),
            ...this.cornerPositions[3].toArray(),
            ...this.cornerPositions[3].toArray(),
            ...this.cornerPositions[2].toArray(),
            ...this.cornerPositions[2].toArray(),
            ...this.cornerPositions[0].toArray(),
            // Top face edges
            ...this.cornerPositions[4].toArray(),
            ...this.cornerPositions[5].toArray(),
            ...this.cornerPositions[5].toArray(),
            ...this.cornerPositions[7].toArray(),
            ...this.cornerPositions[7].toArray(),
            ...this.cornerPositions[6].toArray(),
            ...this.cornerPositions[6].toArray(),
            ...this.cornerPositions[4].toArray(),
            // Vertical edges
            ...this.cornerPositions[0].toArray(),
            ...this.cornerPositions[4].toArray(),
            ...this.cornerPositions[1].toArray(),
            ...this.cornerPositions[5].toArray(),
            ...this.cornerPositions[2].toArray(),
            ...this.cornerPositions[6].toArray(),
            ...this.cornerPositions[3].toArray(),
            ...this.cornerPositions[7].toArray()
        ]);

        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));

        const material = new THREE.LineBasicMaterial({
            color: 0xffffff,
            transparent: true,
            opacity: 0.5
        });

        this.wireframe = new THREE.LineSegments(geometry, material);
        this.scene.add(this.wireframe);
    }

    private onDragStart = (): void => {
        this.onDragStartCallback?.();
    };

    private onDrag = (): void => {
        this.updateWireframe();
        this.onChanged?.();
    };

    private onDragEnd = (): void => {
        this.onDragEndCallback?.();
    };

    dispose(): void {
        // Remove event listeners
        this.dragControls.removeEventListener('dragstart', this.onDragStart);
        this.dragControls.removeEventListener('drag', this.onDrag);
        this.dragControls.removeEventListener('dragend', this.onDragEnd);
        this.dragControls.dispose();

        // Dispose control points
        this.controlPoints.forEach((cp) => {
            cp.geometry.dispose();
            (cp.material as THREE.Material).dispose();
            this.scene.remove(cp);
        });
        this.controlPoints.length = 0;
        this.cornerPositions.length = 0;

        // Dispose wireframe
        if (this.wireframe) {
            this.scene.remove(this.wireframe);
            this.wireframe.geometry.dispose();
            (this.wireframe.material as THREE.Material).dispose();
            this.wireframe = null;
        }
    }
}
