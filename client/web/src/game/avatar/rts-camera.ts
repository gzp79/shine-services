import * as THREE from 'three';
import { CameraConst } from '../../../constants';
import { EventSubscriptions } from '../../engine/events';
import { VEC3_UNIT_Z } from '../../engine/utils';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../systems/world-reference-system';

export interface IRtsCamera {
    readonly width: number;
    readonly height: number;
    readonly aspect: number;

    readonly position: Readonly<THREE.Vector3>;
    readonly lookAt: Readonly<THREE.Vector3>;
    readonly rotation: number;
    readonly cameraDistance: number;

    screenToWorldPlanePoint(screenX: number, screenY: number, planeZ?: number): THREE.Vector3 | null;
    ndcToWorldPlanePoint(ndcX: number, ndcY: number, planeZ?: number): THREE.Vector3 | null;
}

export class RtsCamera implements IRtsCamera {
    width = 1;
    height = 1;

    private _position = new THREE.Vector3(0, -1200, 2000);
    private _lookAt = new THREE.Vector3(0, 0, 0);
    private _rotation: number = 0;
    private _cameraDistance: number = CameraConst.MIN_DISTANCE;

    readonly camera: THREE.PerspectiveCamera;
    private readonly subscriptions: EventSubscriptions;
    private readonly raycaster = new THREE.Raycaster();

    get aspect(): number {
        return this.camera.aspect;
    }

    constructor(events: EventTarget) {
        this.subscriptions = new EventSubscriptions(events);

        this.camera = new THREE.PerspectiveCamera(50, 1, 1, 50000);
        this.camera.up.set(0, 0, 1);
        this.camera.position.copy(this._position);
        this.camera.lookAt(this._lookAt);

        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    get position(): Readonly<THREE.Vector3> {
        return this._position;
    }

    set position(pos: Readonly<THREE.Vector3>) {
        this._position.copy(pos);
        this.camera.position.copy(this._position);
    }

    get lookAt(): Readonly<THREE.Vector3> {
        return this._lookAt;
    }

    set lookAt(target: Readonly<THREE.Vector3>) {
        this._lookAt.copy(target);
        this.camera.lookAt(this._lookAt);
    }

    get rotation(): number {
        return this._rotation;
    }

    get cameraDistance(): number {
        return this._cameraDistance;
    }

    setViewByTarget(lookAt: Readonly<THREE.Vector3>, rotation: number, cameraDistance: number): void {
        const t = (cameraDistance - CameraConst.MIN_DISTANCE) / (CameraConst.MAX_DISTANCE - CameraConst.MIN_DISTANCE);
        const pitch = CameraConst.MIN_PITCH + t * (CameraConst.MAX_PITCH - CameraConst.MIN_PITCH);

        const height = cameraDistance * Math.sin(pitch);
        const horizontalDist = cameraDistance * Math.cos(pitch);

        this._position.x = lookAt.x + horizontalDist * Math.sin(rotation);
        this._position.y = lookAt.y - horizontalDist * Math.cos(rotation);
        this._position.z = lookAt.z + height;
        this._lookAt.copy(lookAt);
        this._rotation = rotation;
        this._cameraDistance = cameraDistance;

        this.camera.position.copy(this._position);
        this.camera.lookAt(this._lookAt);
    }

    screenToWorldPlanePoint(screenX: number, screenY: number, planeZ = 0): THREE.Vector3 | null {
        const ndcX = (screenX / this.width) * 2 - 1;
        const ndcY = -(screenY / this.height) * 2 + 1;
        return this.ndcToWorldPlanePoint(ndcX, ndcY, planeZ);
    }

    ndcToWorldPlanePoint(ndcX: number, ndcY: number, planeZ = 0): THREE.Vector3 | null {
        this.raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), this.camera);
        const plane = new THREE.Plane(VEC3_UNIT_Z, -planeZ);
        const intersectionPoint = new THREE.Vector3();
        return this.raycaster.ray.intersectPlane(plane, intersectionPoint);
    }

    dispose(): void {
        this.subscriptions.dispose();
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this._position.x += event.deltaPosition.x;
        this._position.y += event.deltaPosition.y;
        this._lookAt.x += event.deltaPosition.x;
        this._lookAt.y += event.deltaPosition.y;

        this.camera.position.copy(this._position);
        //this.camera.lookAt(this._lookAt);
    };
}
