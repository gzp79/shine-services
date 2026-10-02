import type * as THREE from 'three';

export type WorldEntityKind = 'chunk' | 'edge' | 'corner';

/** Identity of a world entity. */
export interface WorldEntityId {
    readonly kind: WorldEntityKind;
    key(): string;
}

/** A world main building blocks (chunk / edge / corner). */
export interface WorldEntity {
    readonly id: WorldEntityId;
    readonly group: THREE.Group;
}
