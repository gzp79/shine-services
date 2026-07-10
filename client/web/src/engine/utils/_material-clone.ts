import * as THREE from 'three';
import { MeshStandardNodeMaterial } from 'three/webgpu';

// Material.clone()/copy() (via NodeMaterial/Material) never copies THREE.MeshStandardMaterial's
// classic properties (color, roughness, metalness, map, ...) — only
// THREE.MeshStandardMaterial.prototype.copy does. Use this instead of `.clone()` whenever a
// MeshStandardNodeMaterial originated from loadGltf/toNodeMaterial, or those properties silently
// reset to constructor defaults.
export function cloneStandardMaterial(source: MeshStandardNodeMaterial): MeshStandardNodeMaterial {
    const mat = source.clone() as MeshStandardNodeMaterial;
    THREE.MeshStandardMaterial.prototype.copy.call(mat, source);
    return mat;
}
