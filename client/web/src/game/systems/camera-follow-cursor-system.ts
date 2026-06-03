import * as THREE from 'three';
import type { RtsCamera } from '../avatar/rts-camera';
import { type IWorldCursor } from '../avatar/world-cursor';
import { CameraConst } from '../constants';
import { EventSubscriptions } from '../engine/events';
import type { GameSystem } from '../game-system';

export class CameraFollowCursorSystem implements GameSystem {
    readonly name: string = 'Camera Follow Cursor';

    private readonly subscriptions: EventSubscriptions;

    private position = new THREE.Vector3();

    constructor(
        private readonly camera: RtsCamera,
        private readonly worldCursor: IWorldCursor,
        events: EventTarget
    ) {
        this.subscriptions = new EventSubscriptions(events);
    }

    /*
        update(deltaTime: number): void {
        const target = this.worldCursor.getCameraTarget();

        const distanceDiff = this.camera.position.distanceTo(target.position);
        const distLerpFactor = this.lerpFactor(deltaTime, distanceDiff);
        const currentPosition = this.camera.position.clone().lerp(target.position, distLerpFactor);

        const lookAtDiff = this.camera.lookAt.distanceTo(target.lookAt);
        const lookAtLerpFactor = this.lerpFactor(deltaTime, lookAtDiff);
        const currentLookAt = this.camera.lookAt.clone().lerp(target.lookAt, lookAtLerpFactor);

        this.camera.position = currentPosition;
        this.camera.lookAt = currentLookAt;
    }*/

    update(deltaTime: number): void {
        const cursor = this.worldCursor;

        const posDiff = this.camera.lookAt.distanceTo(cursor.position);
        const pos = this.camera.lookAt.clone().lerp(cursor.position, this.lerpFactor(deltaTime, posDiff));

        const rotDiff = Math.abs(cursor.rotation - this.camera.rotation);
        const rot =
            this.camera.rotation + (cursor.rotation - this.camera.rotation) * this.lerpFactor(deltaTime, rotDiff);

        const distDiff = Math.abs(cursor.cameraDistance - this.camera.cameraDistance);
        const dist =
            this.camera.cameraDistance +
            (cursor.cameraDistance - this.camera.cameraDistance) * this.lerpFactor(deltaTime, distDiff);

        this.camera.setViewByTarget(pos, rot, dist);
    }

    dispose(): void {
        this.subscriptions.dispose();
    }

    private lerpFactor(deltaTime: number, diff: number): number {
        const distanceFactor = Math.min(diff / 1000, 2);
        const distLerpSpeed = CameraConst.BASE_LERP + distanceFactor * CameraConst.LERP_DISTANCE_FACTOR;
        const distLerpFactor = 1 - Math.pow(1 - distLerpSpeed, deltaTime * 6);
        return distLerpFactor;
    }
}
