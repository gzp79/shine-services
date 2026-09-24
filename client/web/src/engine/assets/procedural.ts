import * as THREE from 'three';
import { color } from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import { share } from '../resources/ownership';
import { type ModelSet } from './model-set';

// Procedurally generated assets. A generator produces a ModelSet on demand — the same result a
// fetched asset decodes to — so a catalog can serve it through generate() exactly as it serves a
// fetched one through url(). Resources are shared: the store owns them, consumers only borrow.
// This is the single place procedural assets are named.

export type ProceduralGenerator = () => ModelSet;

function makeMaterial(hex: number): MeshStandardNodeMaterial {
    const m = new MeshStandardNodeMaterial({ roughness: 0.6, metalness: 0.2, side: THREE.DoubleSide });
    m.colorNode = color(hex);
    return m;
}

// Concatenates geometries (position + normal + index) into one buffer, returning the merged
// geometry and the [start, end) index range of each source. Sources are read but not disposed.
function mergeGeometries(geos: THREE.BufferGeometry[]): { geometry: THREE.BufferGeometry; ranges: [number, number][] } {
    let totalVerts = 0;
    let totalIndices = 0;
    for (const g of geos) {
        totalVerts += g.attributes.position.count;
        totalIndices += g.index!.count;
    }

    const positions = new Float32Array(totalVerts * 3);
    const normals = new Float32Array(totalVerts * 3);
    const indices = new Uint32Array(totalIndices);
    const ranges: [number, number][] = [];

    let vOffset = 0;
    let iOffset = 0;
    for (const g of geos) {
        positions.set(g.attributes.position.array as Float32Array, vOffset * 3);
        normals.set(g.attributes.normal.array as Float32Array, vOffset * 3);
        const src = g.index!.array;
        const start = iOffset;
        for (let i = 0; i < src.length; i++) indices[iOffset + i] = src[i] + vOffset;
        iOffset += src.length;
        ranges.push([start, iOffset]);
        vOffset += g.attributes.position.count;
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    return { geometry, ranges };
}

interface Segment {
    color: number;
    geo: THREE.BufferGeometry;
}

interface PartSpec {
    name: string;
    segments: Segment[];
}

// Packs specs into a shared-resource ModelSet: one model per spec, one submesh per segment over its
// index range in the merged geometry. Consumes (disposes) the source segment geometries.
function packModelSet(specs: PartSpec[]): ModelSet {
    const segments = specs.flatMap((s) => s.segments);
    const { geometry, ranges } = mergeGeometries(segments.map((s) => s.geo));
    for (const s of segments) s.geo.dispose();

    let i = 0;
    const models = specs.map((spec) => ({
        name: spec.name,
        parts: spec.segments.map((seg) => {
            const [indexStart, indexEnd] = ranges[i++];
            return { material: share(makeMaterial(seg.color)), indexStart, indexEnd };
        })
    }));
    return { geometry: share(geometry), models };
}

// A sphere, box and torus, each occupying the unit cube [0,1]^3.
function buildShapes(): ModelSet {
    const sphere = new THREE.SphereGeometry(0.4, 16, 12);
    sphere.translate(0.5, 0.5, 0.5);
    const box = new THREE.BoxGeometry(1, 1, 1, 2, 2, 2);
    box.translate(0.5, 0.5, 0.5);
    const torus = new THREE.TorusGeometry(0.3, 0.12, 12, 24);
    torus.translate(0.5, 0.5, 0.5);

    return packModelSet([
        { name: 'sphere', segments: [{ color: 0x4488cc, geo: sphere }] },
        { name: 'box', segments: [{ color: 0xcc4444, geo: box }] },
        { name: 'torus', segments: [{ color: 0x44cc88, geo: torus }] }
    ]);
}

// Two box heights (low / high) rising from z=0, filling the unit cube's 2x2 footprint. Each shape
// picks a height per quarter: 'x' = down (low), '_' = up (high). Read top row then bottom row; the
// top text row is the far (higher y) edge. Colored by height, not by shape, so the same low/high
// color pair reads consistently across every variant.
const LOW = 0.5;
const HIGH = 1.0;
const LOW_COLOR = 0x3366cc; // blue
const HIGH_COLOR = 0x44aa55; // green
const HEIGHT_SHAPES: { name: string; rows: [string, string] }[] = [
    { name: 'flat-low', rows: ['xx', 'xx'] },
    { name: 'corner', rows: ['x_', '__'] },
    { name: 'diagonal', rows: ['x_', '_x'] },
    { name: 'left-step', rows: ['x_', 'x_'] },
    { name: 'flat-high', rows: ['__', '__'] },
    { name: 'notch', rows: ['x_', 'xx'] }
];

// One geometry per height present in the shape (at most two: low and high), each merging the
// quarters sharing that height. Kept separate so they can carry distinct colors as submeshes.
function buildHeightBoxShape(rows: [string, string]): Segment[] {
    const byHeight = new Map<number, THREE.BufferGeometry[]>();
    for (let line = 0; line < 2; line++) {
        for (let col = 0; col < 2; col++) {
            const height = rows[line][col] === 'x' ? LOW : HIGH;
            const row = 1 - line; // top text row = far (higher y) footprint row
            const box = new THREE.BoxGeometry(0.5, 0.5, height);
            box.translate(col * 0.5 + 0.25, row * 0.5 + 0.25, height / 2);
            const boxes = byHeight.get(height) ?? [];
            boxes.push(box);
            byHeight.set(height, boxes);
        }
    }
    return [...byHeight.entries()].map(([height, boxes]) => {
        const { geometry } = mergeGeometries(boxes);
        for (const b of boxes) b.dispose();
        return { color: height === LOW ? LOW_COLOR : HIGH_COLOR, geo: geometry };
    });
}

function buildHeightBoxes(): ModelSet {
    return packModelSet(HEIGHT_SHAPES.map((s) => ({ name: s.name, segments: buildHeightBoxShape(s.rows) })));
}

export const PROCEDURAL_ASSETS: Record<string, ProceduralGenerator> = {
    'generated-shapes': buildShapes,
    'generated-tile': buildHeightBoxes
};
