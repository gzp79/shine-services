import * as THREE from 'three';
import type { WebGPURenderer } from 'three/webgpu';

/**
 * Renders an interleaved NDC [x0,y0,x1,y1,...] point path as a Three.js line
 * in a dedicated orthographic scene, drawn after the main scene pass.
 *
 * NDC convention: center=(0,0), x right, y up, range [-1,1].
 * The ortho camera maps that range directly to clip space, so no extra math needed.
 *
 * Usage: call render(renderer) from an onPostRender callback.
 */
export class StrokeLineNode {
    private readonly scene: THREE.Scene;
    private readonly camera: THREE.OrthographicCamera;
    private readonly geometry: THREE.BufferGeometry;
    private readonly positions: Float32Array;
    private readonly posAttr: THREE.BufferAttribute;
    private readonly line: THREE.Line;

    constructor(capacity: number, color = 0xff4500, linewidth = 2) {
        // ortho camera covering exactly [-1,1] in both axes — matches NDC directly
        this.camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

        this.scene = new THREE.Scene();

        // z=0 for all points; pre-fill with 0
        this.positions = new Float32Array(capacity * 3);
        this.posAttr = new THREE.BufferAttribute(this.positions, 3);
        this.posAttr.setUsage(THREE.DynamicDrawUsage);

        this.geometry = new THREE.BufferGeometry();
        this.geometry.setAttribute('position', this.posAttr);
        this.geometry.setDrawRange(0, 0);

        const material = new THREE.LineBasicMaterial({ color, linewidth });
        this.line = new THREE.Line(this.geometry, material);
        this.line.frustumCulled = false;
        this.scene.add(this.line);
    }

    /** buf: interleaved NDC [x,y,...], count: number of points. */
    update(buf: Float32Array, count: number): void {
        for (let i = 0; i < count; i++) {
            this.positions[i * 3] = buf[i * 2];
            this.positions[i * 3 + 1] = buf[i * 2 + 1];
            this.positions[i * 3 + 2] = 0;
        }
        this.posAttr.needsUpdate = true;
        this.geometry.setDrawRange(0, count);
    }

    clear(): void {
        this.geometry.setDrawRange(0, 0);
    }

    async render(renderer: WebGPURenderer): Promise<void> {
        const prevAutoClear = renderer.autoClear;
        renderer.autoClear = false;
        await renderer.renderAsync(this.scene, this.camera);
        renderer.autoClear = prevAutoClear;
    }

    dispose(): void {
        this.geometry.dispose();
        (this.line.material as THREE.Material).dispose();
    }
}
