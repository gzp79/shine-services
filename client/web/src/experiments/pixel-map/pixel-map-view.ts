import * as THREE from 'three';
import { cellAtPoint, cellCenter, paletteRgba } from './pixel-map-layout';

// What the view reads from the stream of the pixel map (the wasm PixelMapStream).
export interface PixelMapSource {
    revision(): number;
    colors(): Uint8Array;
    pending_mask(): Uint8Array;
    width(): number;
    height(): number;
}

const MARKER_Z = 0.05;
const MARKER_PULSE_HZ = 0.8;
const ETHER_POINTS = 600;
const ETHER_FADE_PER_SECOND = 4;
const DIMMED_MAP = 0.35;

// The map as a texture, a pulsing frame on the cells that are not confirmed by the server yet, and an
// "ether" of drifting particles over the map while there is no live connection.
export class PixelMapView {
    private readonly texture: THREE.DataTexture;
    private readonly pixels: Uint8Array;
    private readonly mapMaterial: THREE.MeshBasicMaterial;
    private readonly map: THREE.Mesh;
    private readonly markers: THREE.InstancedMesh;
    private readonly markerMaterial: THREE.MeshBasicMaterial;
    private readonly ether: THREE.Points;
    private readonly etherPhase: Float32Array;
    private readonly etherBase: Float32Array;
    private readonly matrix = new THREE.Matrix4();
    private etherAmount = 0;
    private lastRevision = -1;
    private width = 0;
    private height = 0;

    constructor(
        private readonly scene: THREE.Scene,
        width: number,
        height: number
    ) {
        this.width = width;
        this.height = height;
        this.pixels = new Uint8Array(width * height * 4);
        this.texture = new THREE.DataTexture(this.pixels, width, height, THREE.RGBAFormat);
        this.texture.magFilter = THREE.NearestFilter;
        this.texture.minFilter = THREE.NearestFilter;
        this.texture.colorSpace = THREE.SRGBColorSpace;
        this.texture.generateMipmaps = false;
        this.texture.needsUpdate = true;

        this.mapMaterial = new THREE.MeshBasicMaterial({ map: this.texture });
        this.map = new THREE.Mesh(new THREE.PlaneGeometry(width, height), this.mapMaterial);
        this.scene.add(this.map);

        this.markerMaterial = new THREE.MeshBasicMaterial({
            color: 0xffffff,
            transparent: true,
            depthTest: false,
            side: THREE.DoubleSide
        });
        this.markers = new THREE.InstancedMesh(
            new THREE.RingGeometry(0.28, 0.44, 4, 1, Math.PI / 4),
            this.markerMaterial,
            width * height
        );
        this.markers.count = 0;
        this.markers.frustumCulled = false;
        this.scene.add(this.markers);

        this.etherPhase = new Float32Array(ETHER_POINTS);
        this.etherBase = new Float32Array(ETHER_POINTS * 2);
        const positions = new Float32Array(ETHER_POINTS * 3);
        for (let i = 0; i < ETHER_POINTS; i++) {
            this.etherPhase[i] = Math.random() * Math.PI * 2;
            this.etherBase[i * 2] = (Math.random() - 0.5) * width * 1.2;
            this.etherBase[i * 2 + 1] = (Math.random() - 0.5) * height * 1.2;
        }
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        this.ether = new THREE.Points(
            geometry,
            new THREE.PointsMaterial({
                color: 0x9ad7ff,
                size: 0.45,
                transparent: true,
                opacity: 0,
                blending: THREE.AdditiveBlending,
                depthWrite: false
            })
        );
        this.ether.frustumCulled = false;
        this.ether.visible = false;
        this.scene.add(this.ether);
    }

    // The world cell under a point of the map plane, or null off the map.
    cellAt(point: THREE.Vector3): { x: number; y: number } | null {
        return cellAtPoint(point.x, point.y, this.width, this.height);
    }

    // The mesh to pick against.
    get pickTarget(): THREE.Object3D {
        return this.map;
    }

    // `etherOn`: no live connection, so what is shown may be stale.
    update(source: PixelMapSource, etherOn: boolean, time: number, dt: number): void {
        const revision = source.revision();
        if (revision !== this.lastRevision) {
            this.lastRevision = revision;
            this.upload(source);
        }
        this.animateMarkers(time);
        this.animateEther(etherOn, time, dt);
    }

    dispose(): void {
        this.scene.remove(this.map, this.markers, this.ether);
        this.map.geometry.dispose();
        this.mapMaterial.dispose();
        this.texture.dispose();
        this.markers.geometry.dispose();
        this.markers.dispose();
        this.markerMaterial.dispose();
        this.ether.geometry.dispose();
        (this.ether.material as THREE.Material).dispose();
    }

    private upload(source: PixelMapSource): void {
        const colors = source.colors();
        const pending = source.pending_mask();
        const count = this.width * this.height;
        for (let i = 0; i < count; i++) {
            this.pixels.set(paletteRgba(colors[i]), i * 4);
        }
        this.texture.needsUpdate = true;

        let marked = 0;
        for (let i = 0; i < count; i++) {
            if (!pending[i]) continue;
            const [x, y] = cellCenter(i % this.width, Math.floor(i / this.width), this.width, this.height);
            this.matrix.makeTranslation(x, y, MARKER_Z);
            this.markers.setMatrixAt(marked++, this.matrix);
        }
        this.markers.count = marked;
        this.markers.instanceMatrix.needsUpdate = true;
    }

    private animateMarkers(time: number): void {
        this.markers.visible = this.markers.count > 0;
        this.markerMaterial.opacity = 0.55 + 0.45 * Math.sin(time * Math.PI * 2 * MARKER_PULSE_HZ);
    }

    private animateEther(on: boolean, time: number, dt: number): void {
        const target = on ? 1 : 0;
        this.etherAmount += (target - this.etherAmount) * Math.min(1, dt * ETHER_FADE_PER_SECOND);
        if (Math.abs(target - this.etherAmount) < 0.01) this.etherAmount = target;

        const dimmed = 1 - (1 - DIMMED_MAP) * this.etherAmount;
        this.mapMaterial.color.setScalar(dimmed);
        this.ether.visible = this.etherAmount > 0;
        if (!this.ether.visible) return;

        (this.ether.material as THREE.PointsMaterial).opacity = 0.8 * this.etherAmount;
        const position = this.ether.geometry.getAttribute('position') as THREE.BufferAttribute;
        for (let i = 0; i < ETHER_POINTS; i++) {
            const phase = this.etherPhase[i] + time * 0.6;
            position.setXYZ(
                i,
                this.etherBase[i * 2] + Math.sin(phase) * 1.5,
                this.etherBase[i * 2 + 1] + Math.cos(phase * 0.8) * 1.5,
                0.3 + Math.sin(phase * 1.7) * 0.4
            );
        }
        position.needsUpdate = true;
    }
}
