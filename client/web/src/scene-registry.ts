import { WebGPURenderer } from 'three/webgpu';
import type { Application } from './engine/application';
import type { AssetCatalogBuilder } from './engine/assets/catalog';
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
    create: (container: HTMLElement, renderer: WebGPURenderer, catalogBuilder: AssetCatalogBuilder) => Application;
}

// Single source of truth for selectable scenes. Add an experiment with one entry.
export const scenes: SceneEntry[] = [
    { id: '', title: 'Game', create: (c, r, cb) => new Game(c, r, cb) },
    { id: 'hex-mesh', title: 'Hex Mesh', create: (c, r, cb) => new HexMesh(c, r, cb) },
    { id: 'cdt', title: 'CDT', create: (c, r, cb) => new Cdt(c, r, cb) },
    { id: 'input-events', title: 'Input Events', create: (c, r, cb) => new InputControl(c, r, cb) },
    { id: 'trilinear', title: 'Trilinear', create: (c, r, cb) => new Trilinear(c, r, cb) },
    { id: 'world-neighbors', title: 'World Neighbors', create: (c, r, cb) => new WorldNeighbors(c, r, cb) },
    { id: 'tile-chunk', title: 'Tile Chunk', create: (c, r, cb) => new TileChunk(c, r, cb) },
    {
        id: 'instanced-color-mesh',
        title: 'Instanced Color Mesh',
        create: (c, r, cb) => new InstancedColorMeshExp(c, r, cb)
    },
    { id: 'asset-viewer', title: 'Asset Viewer', create: (c, r, cb) => new AssetViewer(c, r, cb) }
];

const sceneById = new Map(scenes.map((s) => [s.id, s]));

export function createContent(
    id: string,
    container: HTMLElement,
    renderer: WebGPURenderer,
    catalogBuilder: AssetCatalogBuilder
): Application {
    const entry = sceneById.get(id) ?? sceneById.get('')!;
    return entry.create(container, renderer, catalogBuilder);
}
