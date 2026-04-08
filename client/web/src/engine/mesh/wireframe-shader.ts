import * as THREE from 'three';

/**
 * Wireframe shader with perspective-correct edge rendering.
 * Uses barycentric coordinates and edge flags to render only real edges (no diagonals).
 */
export const wireframeVertexShader = `
    attribute vec3 barycentric;
    attribute vec3 edgeFlags;
    attribute vec3 color;

    varying vec3 vBarycentric;
    varying vec3 vEdgeFlags;
    varying vec3 vColor;
    varying float vDepth;

    void main() {
        vBarycentric = barycentric;
        vEdgeFlags = edgeFlags;
        vColor = color;

        vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
        vDepth = -mvPosition.z; // Camera-space depth

        gl_Position = projectionMatrix * mvPosition;
    }
`;

export const wireframeFragmentShader = `
    uniform vec3 color;
    uniform float edgeWidth;
    uniform float opacity;
    uniform float glowPower;

    varying vec3 vBarycentric;
    varying vec3 vEdgeFlags;
    varying vec3 vColor;
    varying float vDepth;

    void main() {
        // Calculate screen-space derivatives
        vec3 d = fwidth(vBarycentric);

        // Distance to each edge in pixels (barycentric = 0 at edges)
        vec3 edgeDistance = vBarycentric / d;

        // Mask out disabled edges (flag = 0 means ignore that edge)
        vec3 maskedDistance = mix(vec3(999999.0), edgeDistance, vEdgeFlags);

        // Find closest enabled edge
        float minDistance = min(min(maskedDistance.x, maskedDistance.y), maskedDistance.z);

        // Convert to edge intensity (1 = on edge, 0 = far from edge)
        float edgeIntensity = 1.0 - smoothstep(0.0, edgeWidth, minDistance);

        // Alpha test
        if (edgeIntensity < 0.01) discard;

        // Glow effect controlled by glowPower
        float glow = pow(edgeIntensity, glowPower);

        // Combine uniform color with vertex color
        vec3 baseColor = color * vColor;

        // Apply glow brightness
        vec3 finalColor = baseColor * (0.7 + glow * 0.8);

        // Depth-based opacity: farther edges are dimmer
        float depthFade = clamp(1.0 - vDepth / 5000.0, 0.5, 1.0);
        float alpha = edgeIntensity * opacity * depthFade;

        gl_FragColor = vec4(finalColor, alpha);
    }
`;

/**
 * Create wireframe shader material.
 */
export function createWireframeMaterial(color: THREE.Color | number): THREE.ShaderMaterial {
    const colorObj = color instanceof THREE.Color ? color : new THREE.Color(color);

    return new THREE.ShaderMaterial({
        uniforms: {
            color: { value: colorObj },
            edgeWidth: { value: 3.0 }, // Screen-space pixels - sharp core
            opacity: { value: 1.0 }, // Opaque core
            glowPower: { value: 1.0 } // No glow for sharp edge
        },
        vertexShader: wireframeVertexShader,
        fragmentShader: wireframeFragmentShader,
        transparent: false,
        depthTest: true,
        depthWrite: false,
        side: THREE.DoubleSide
    });
}

/**
 * Create glow wireframe material (wider, blurred, transparent).
 */
export function createWireframeGlowMaterial(color: THREE.Color | number, opacity: number = 0.4): THREE.ShaderMaterial {
    const colorObj = color instanceof THREE.Color ? color : new THREE.Color(color);

    return new THREE.ShaderMaterial({
        uniforms: {
            color: { value: colorObj },
            edgeWidth: { value: 18.0 }, // Wider for glow
            opacity: { value: opacity },
            glowPower: { value: 0.5 } // Strong glow falloff
        },
        vertexShader: wireframeVertexShader,
        fragmentShader: wireframeFragmentShader,
        transparent: true,
        depthTest: true,
        depthWrite: false,
        side: THREE.DoubleSide
    });
}
