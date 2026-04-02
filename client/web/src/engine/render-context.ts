import * as THREE from 'three';
import { EventDispatcher } from './events';

// ViewportResizeEvent - THe size of the render canvas has changed
export const VIEWPORT_RESIZE = 'viewportresize';
export type ViewportResizeEvent = {
    width: number;
    height: number;
};

export class RenderContext {
    readonly scene: THREE.Scene;
    readonly renderer: THREE.WebGLRenderer;
    readonly domElement: HTMLElement;
    private readonly resizeObserver: ResizeObserver;
    private readonly dispatcher: EventDispatcher;
    private _width: number;
    private _height: number;
    private readonly canvas: HTMLCanvasElement;
    private raycaster = new THREE.Raycaster();
    private mousePosition = new THREE.Vector2(-1, -1);
    private currentHoveredChunk: any = null; // Chunk type (avoid circular dependency)

    get width(): number {
        return this._width;
    }

    get height(): number {
        return this._height;
    }

    constructor(container: HTMLElement, events: EventTarget) {
        this.dispatcher = new EventDispatcher(events);
        this.domElement = container;

        // Create scene
        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x1a1a2e);

        // Create renderer
        const width = container.clientWidth;
        const height = container.clientHeight;
        this._width = width;
        this._height = height;

        this.renderer = new THREE.WebGLRenderer({ antialias: true });
        this.renderer.setSize(width, height);
        this.renderer.setPixelRatio(window.devicePixelRatio);
        container.appendChild(this.renderer.domElement);
        this.canvas = this.renderer.domElement;

        // Lighting
        const ambient = new THREE.AmbientLight(0xffffff, 0.6);
        this.scene.add(ambient);

        const directional = new THREE.DirectionalLight(0xffffff, 0.8);
        directional.position.set(1000, -500, 3000);
        this.scene.add(directional);

        // Resize handling
        this.resizeObserver = new ResizeObserver(() => {
            const w = container.clientWidth;
            const h = container.clientHeight;
            this._width = w;
            this._height = h;
            this.renderer.setSize(w, h);
            this.dispatcher.dispatch<ViewportResizeEvent>(VIEWPORT_RESIZE, {
                width: w,
                height: h
            });
        });
        this.resizeObserver.observe(container);

        // Mouse event listeners
        this.canvas.addEventListener('mousemove', this.onMouseMove);
        this.canvas.addEventListener('mouseleave', this.onMouseLeave);
    }

    private onMouseMove = (event: MouseEvent): void => {
        const rect = this.canvas.getBoundingClientRect();
        this.mousePosition.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
        this.mousePosition.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
    };

    private onMouseLeave = (): void => {
        this.mousePosition.set(-1, -1);
        if (this.currentHoveredChunk) {
            this.currentHoveredChunk.hideSelection();
            this.currentHoveredChunk = null;
        }
    };

    private updateSelectionHover(camera: THREE.PerspectiveCamera): void {
        // Skip if mouse is outside canvas
        if (this.mousePosition.x === -1 && this.mousePosition.y === -1) {
            return;
        }

        // Raycast against all visible objects
        this.raycaster.setFromCamera(this.mousePosition, camera);
        const intersects = this.raycaster.intersectObjects(this.scene.children, true);

        // Find first chunk hit (chunk is on grandparent due to MeshBuilder.group wrapper)
        let hitChunk = null;
        let hitPoint = null;

        for (const intersect of intersects) {
            const obj = intersect.object;
            // Check parent first, then grandparent (for MeshBuilder hierarchy)
            const chunk = obj.parent?.userData.chunk || obj.parent?.parent?.userData.chunk;
            if (chunk) {
                hitChunk = chunk;
                hitPoint = intersect.point;
                break;
            }
        }

        // Update hover state - delegate to chunks
        if (hitChunk !== this.currentHoveredChunk) {
            // Hide selection on previous chunk
            if (this.currentHoveredChunk) {
                this.currentHoveredChunk.hideSelection();
            }
            this.currentHoveredChunk = hitChunk;
        }

        // Show selection at hit point
        if (hitChunk && hitPoint) {
            hitChunk.showSelectionAt(hitPoint);
        }
    }

    render(camera: THREE.PerspectiveCamera): void {
        this.updateSelectionHover(camera);
        this.renderer.render(this.scene, camera);
    }

    dispose(): void {
        this.canvas.removeEventListener('mousemove', this.onMouseMove);
        this.canvas.removeEventListener('mouseleave', this.onMouseLeave);
        this.resizeObserver.disconnect();
        this.renderer.dispose();
        this.renderer.domElement.remove();
    }
}
