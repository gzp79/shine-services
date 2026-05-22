import * as THREE from 'three';
import { attribute, clamp, float, fwidth, min, mix, pow, smoothstep, uniform, vec3 } from 'three/tsl';
import { MeshBasicNodeMaterial } from 'three/webgpu';

function buildWireframeMaterial(
    colorValue: THREE.Color,
    edgeWidth: number,
    opacity: number,
    glowPower: number,
    transparent: boolean
): MeshBasicNodeMaterial {
    const uColor = uniform(colorValue);
    const uEdgeWidth = uniform(float(edgeWidth));
    const uOpacity = uniform(float(opacity));
    const uGlowPower = uniform(float(glowPower));

    const barycentric = attribute('barycentric', 'vec3');
    const edgeFlags = attribute('edgeFlags', 'vec3');
    const vColor = attribute('color', 'vec3');

    const d = fwidth(barycentric);
    const edgeDistance = barycentric.div(d);
    const maskedDistance = mix(vec3(999999.0), edgeDistance, edgeFlags);
    const minDistance = min(min(maskedDistance.x, maskedDistance.y), maskedDistance.z);
    const edgeIntensity = float(1.0).sub(smoothstep(float(0.0), uEdgeWidth, minDistance));
    const glow = pow(edgeIntensity, uGlowPower);
    const baseColor = uColor.mul(vColor);
    const finalColor = baseColor.mul(float(0.7).add(glow.mul(0.8)));
    const alpha = edgeIntensity.mul(uOpacity);

    const material = new MeshBasicNodeMaterial();
    material.colorNode = vec3(finalColor.x, finalColor.y, finalColor.z);
    material.opacityNode = alpha;
    material.transparent = transparent;
    material.alphaTest = 0.01;
    material.side = THREE.DoubleSide;
    material.depthTest = true;
    material.depthWrite = false;

    return material;
}

export function createWireframeMaterial(color: THREE.Color | number): MeshBasicNodeMaterial {
    const colorObj = color instanceof THREE.Color ? color : new THREE.Color(color);
    return buildWireframeMaterial(colorObj, 3.0, 1.0, 1.0, false);
}

export function createWireframeGlowMaterial(color: THREE.Color | number, opacity: number = 0.4): MeshBasicNodeMaterial {
    const colorObj = color instanceof THREE.Color ? color : new THREE.Color(color);
    return buildWireframeMaterial(colorObj, 18.0, opacity, 0.5, true);
}
