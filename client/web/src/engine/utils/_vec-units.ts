import * as THREE from 'three';

export const VEC2_ZERO: Readonly<THREE.Vector2> = Object.freeze(new THREE.Vector2(0, 0));
export const VEC2_UNIT_X: Readonly<THREE.Vector2> = Object.freeze(new THREE.Vector2(1, 0));
export const VEC2_UNIT_Y: Readonly<THREE.Vector2> = Object.freeze(new THREE.Vector2(0, 1));

export const VEC3_ZERO: Readonly<THREE.Vector3> = Object.freeze(new THREE.Vector3(0, 0, 0));
export const VEC3_UNIT_X: Readonly<THREE.Vector3> = Object.freeze(new THREE.Vector3(1, 0, 0));
export const VEC3_UNIT_Y: Readonly<THREE.Vector3> = Object.freeze(new THREE.Vector3(0, 1, 0));
export const VEC3_UNIT_Z: Readonly<THREE.Vector3> = Object.freeze(new THREE.Vector3(0, 0, 1));
