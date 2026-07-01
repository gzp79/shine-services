import * as THREE from 'three';
import { CameraConst } from '../../constants';
import { EventSubscriptions } from '../../engine/events';
import { ManagedMesh } from '../../engine/render/managed-mesh';
import { GameResource } from '../game-resource';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../systems/world-reference-system';

export interface IWorldCursor {
    readonly position: Readonly<THREE.Vector3>;
    readonly rotation: number;
    readonly direction: Readonly<THREE.Vector3>;
    readonly cameraDistance: number;
}

export class WorldCursor implements GameResource, IWorldCursor {
    readonly name: string = 'WorldCursor';

    readonly position = new THREE.Vector3(0, 0, 0);
    rotation = 0;
    readonly direction = new THREE.Vector3(0, 1, 0);
    cameraDistance = 600;

    private parent: THREE.Object3D;
    private mesh: ManagedMesh | null = null;
    private readonly subscriptions: EventSubscriptions;

    constructor(parent: THREE.Object3D, events: EventTarget) {
        this.parent = parent;

        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    get showMesh(): boolean {
        return this.mesh?.visible ?? false;
    }

    set showMesh(value: boolean) {
        if (value) {
            this.createMesh();
        } else {
            this.disposeMesh();
        }
    }

    setPosition(pos: THREE.Vector3Like): void {
        this.position.copy(pos);
        if (this.mesh) {
            this.mesh.position.copy(this.position);
        }
    }

    moveBy(forward: number, side: number, up: number): void {
        this.position.x += forward * this.direction.x + side * this.direction.y;
        this.position.y += forward * this.direction.y - side * this.direction.x;
        this.position.z += up;
        if (this.mesh) {
            this.mesh.position.copy(this.position);
        }
    }

    setRotation(angle: number): void {
        this.rotation = angle;
        this.direction.set(-Math.sin(angle), Math.cos(angle), 0);
        if (this.mesh) {
            this.mesh.rotation.z = angle;
        }
    }

    rotateBy(delta: number): void {
        this.setRotation(this.rotation + delta);
    }

    setZoom(distance: number): void {
        this.cameraDistance = Math.max(CameraConst.MIN_DISTANCE, Math.min(CameraConst.MAX_DISTANCE, distance));
    }

    zoomBy(delta: number): void {
        const midDistance = (CameraConst.MAX_DISTANCE + CameraConst.MIN_DISTANCE) / 2;
        const zoomScale = (this.cameraDistance / midDistance) * CameraConst.ZOOM_DISTANCE_SCALE;
        this.setZoom(this.cameraDistance + delta * zoomScale);
    }

    dispose(): void {
        this.disposeMesh();
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this.setPosition({
            x: this.position.x + event.deltaPosition.x,
            y: this.position.y + event.deltaPosition.y,
            z: this.position.z
        });
    };

    private createMesh(): void {
        if (this.mesh) return;

        const geometry = new THREE.BufferGeometry();
        const headLength = 30;
        const baseWidth = 15;
        const vertices = new Float32Array([0, headLength, 0, -baseWidth, -5, 0, baseWidth, -5, 0]);
        geometry.setAttribute('position', new THREE.BufferAttribute(vertices, 3));
        geometry.setIndex([0, 1, 2]);

        const material = new THREE.MeshBasicMaterial({
            color: 0x0000ff,
            side: THREE.DoubleSide,
            depthTest: false,
            depthWrite: false
        });
        this.mesh = ManagedMesh.own(geometry, material);
        this.mesh.renderOrder = 998;
        this.mesh.frustumCulled = false;
        this.mesh.position.copy(this.position);
        this.mesh.rotation.z = this.rotation;

        this.parent.add(this.mesh);
    }

    private disposeMesh(): void {
        if (!this.mesh) return;

        this.parent.remove(this.mesh);
        this.mesh.dispose();
        this.mesh = null;
    }
}
