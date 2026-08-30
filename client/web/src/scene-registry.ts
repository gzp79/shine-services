import { WebGPURenderer } from 'three/webgpu';
import type { Application } from './engine/application';
import { AssetViewer } from './experiments/asset-viewer/index';
import { Cdt } from './experiments/cdt/index';
import { HexMesh } from './experiments/hex-mesh/index';
import { InputControl } from './experiments/input-control/index';
import { InstancedColorMeshExp } from './experiments/instanced-color-mesh/index';
import { TileChunk } from './experiments/tile-chunk/index';
import { Trilinear } from './experiments/trilinear/index';
import { WorldNeighbors } from './experiments/world-neighbors/index';
import { Game } from './game/game';

export interface SceneEntry {
    id: string; // '' = the shipped game
    title: string;
    create: (container: HTMLElement, renderer: WebGPURenderer) => Application;
}

// Single source of truth for selectable scenes. Add an experiment with one entry.
export const scenes: SceneEntry[] = [
    { id: '', title: 'Game', create: (c, r) => new Game(c, r) },
    { id: 'hex-mesh', title: 'Hex Mesh', create: (c, r) => new HexMesh(c, r) },
    { id: 'cdt', title: 'CDT', create: (c, r) => new Cdt(c, r) },
    { id: 'input-events', title: 'Input Events', create: (c, r) => new InputControl(c, r) },
    { id: 'trilinear', title: 'Trilinear', create: (c, r) => new Trilinear(c, r) },
    { id: 'world-neighbors', title: 'World Neighbors', create: (c, r) => new WorldNeighbors(c, r) },
    { id: 'tile-chunk', title: 'Tile Chunk', create: (c, r) => new TileChunk(c, r) },
    { id: 'instanced-color-mesh', title: 'Instanced Color Mesh', create: (c, r) => new InstancedColorMeshExp(c, r) },
    { id: 'asset-viewer', title: 'Asset Viewer', create: (c, r) => new AssetViewer(c, r) }
];

const sceneById = new Map(scenes.map((s) => [s.id, s]));

export function createContent(id: string, container: HTMLElement, renderer: WebGPURenderer): Application {
    const entry = sceneById.get(id) ?? sceneById.get('')!;
    return entry.create(container, renderer);
}
