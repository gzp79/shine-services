import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { EventSubscriptions } from '../engine/events';
import type { RenderContext } from '../engine/render-context';
import { VIEWPORT_RESIZE, type ViewportResizeEvent } from '../engine/render-context';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../systems/world-reference-system';

export class Camera {
    readonly camera: THREE.PerspectiveCamera;
    readonly worldPosition = new THREE.Vector2(0, 0);
    private readonly controls: OrbitControls;
    private readonly renderContext: RenderContext;
    private centerDot: THREE.Points | null = null;
    private readonly subscriptions: EventSubscriptions;
    private freezeDotPosition = false;

    constructor(renderContext: RenderContext, events: EventTarget) {
        this.renderContext = renderContext;
        this.subscriptions = new EventSubscriptions(events);

        // Create camera
        const aspect = renderContext.width / renderContext.height;
        this.camera = new THREE.PerspectiveCamera(50, aspect, 1, 50000);
        this.camera.up.set(0, 0, 1);
        this.camera.position.set(0, -1200, 2000);
        this.camera.lookAt(0, 0, 0);

        // Create controls
        this.controls = new OrbitControls(this.camera, renderContext.domElement);
        this.controls.target.set(0, 0, 0);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.1;
        this.controls.update();

        this.subscriptions.on<ViewportResizeEvent>(VIEWPORT_RESIZE, this.handleViewportResize);
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);

        // Tab key to freeze/unfreeze center dot position
        this.subscriptions.listenWindow('keydown', (e) => {
            if (e.key === 'Tab') {
                e.preventDefault();
                this.freezeDotPosition = !this.freezeDotPosition;
                this.updateDotColor();
            }
        });

        this.showCenterDot = true;
    }

    get showCenterDot(): boolean {
        return this.centerDot?.visible ?? false;
    }

    set showCenterDot(value: boolean) {
        if (value) {
            this.createCenterDot();
        } else if (!value) {
            this.destroyCenterDot();
        }
    }

    update(): void {
        // Update controls
        this.controls.update();

        // Calculate world position (camera forward ∩ XY plane at z=0)
        this.updateWorldPosition();

        // Update center dot position (if exists and not frozen)
        if (this.centerDot && !this.freezeDotPosition) {
            const posAttr = this.centerDot.geometry.attributes.position as THREE.BufferAttribute;
            posAttr.setXYZ(0, this.worldPosition.x, this.worldPosition.y, 0.1);
            posAttr.needsUpdate = true;
        }
    }

    /**
     * Teleport camera to a world position.
     */
    teleportTo(worldPos: THREE.Vector2): void {
        this.camera.position.x = worldPos.x;
        this.camera.position.y = worldPos.y - 1200; // Offset behind target
        this.controls.target.set(worldPos.x, worldPos.y, 0);
        this.controls.update();
        this.updateWorldPosition();
    }

    destroy(): void {
        this.destroyCenterDot();
        this.controls.dispose();
        this.subscriptions.destroy();
    }

    private createCenterDot(): void {
        if (this.centerDot) return;

        const dotGeometry = new THREE.BufferGeometry();
        const dotPosition = new Float32Array([this.worldPosition.x, this.worldPosition.y, 0.1]);
        dotGeometry.setAttribute('position', new THREE.BufferAttribute(dotPosition, 3));
        const dotMaterial = new THREE.PointsMaterial({
            color: 0xff0000, // Red when tracking
            size: 4,
            sizeAttenuation: false, // Fixed 4px size independent of distance
            depthTest: false, // Always render on top
            depthWrite: false // Don't write to depth buffer
        });
        this.centerDot = new THREE.Points(dotGeometry, dotMaterial);
        this.centerDot.renderOrder = 999; // Render last
        this.centerDot.frustumCulled = false; // Never cull - always render
        this.renderContext.scene.add(this.centerDot);
    }

    private updateDotColor(): void {
        if (!this.centerDot) return;
        const material = this.centerDot.material as THREE.PointsMaterial;
        material.color.setHex(this.freezeDotPosition ? 0x00ff00 : 0xff0000); // Green when frozen, red when tracking
    }

    private handleViewportResize = (event: ViewportResizeEvent): void => {
        this.camera.aspect = event.width / event.height;
        this.camera.updateProjectionMatrix();
    };

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this.camera.position.x += event.deltaPosition.x;
        this.camera.position.y += event.deltaPosition.y;
        this.controls.target.x += event.deltaPosition.x;
        this.controls.target.y += event.deltaPosition.y;
    };

    private updateWorldPosition(): void {
        const camPos = this.camera.position;
        const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(this.camera.quaternion);

        // Check if camera is parallel to plane
        if (Math.abs(forward.z) < 0.001) {
            // Fallback: use camera XY position
            this.worldPosition.set(camPos.x, camPos.y);
        } else {
            // Intersect with z=0 plane: point = camPos + t * forward, where t = -camPos.z / forward.z
            const t = -camPos.z / forward.z;
            this.worldPosition.set(camPos.x + t * forward.x, camPos.y + t * forward.y);
        }
    }

    private destroyCenterDot(): void {
        if (!this.centerDot) return;

        this.centerDot.parent?.remove(this.centerDot);
        this.centerDot.geometry.dispose();
        (this.centerDot.material as THREE.Material).dispose();
        this.centerDot = null;
    }
}
