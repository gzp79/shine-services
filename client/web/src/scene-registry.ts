import type { Scene, SceneContext } from './engine/scene';
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
    create: (context: SceneContext) => Scene;
}

// Single source of truth for selectable scenes. Add an experiment with one entry.
export const scenes: SceneEntry[] = [
    { id: '', title: 'Game', create: (context) => new Game(context) },
    { id: 'hex-mesh', title: 'Hex Mesh', create: (context) => new HexMesh(context) },
    { id: 'cdt', title: 'CDT', create: (context) => new Cdt(context) },
    { id: 'input-events', title: 'Input Events', create: (context) => new InputControl(context) },
    { id: 'trilinear', title: 'Trilinear', create: (context) => new Trilinear(context) },
    { id: 'world-neighbors', title: 'World Neighbors', create: (context) => new WorldNeighbors(context) },
    { id: 'tile-chunk', title: 'Tile Chunk', create: (context) => new TileChunk(context) },
    {
        id: 'instanced-color-mesh',
        title: 'Instanced Color Mesh',
        create: (context) => new InstancedColorMeshExp(context)
    },
    { id: 'asset-viewer', title: 'Asset Viewer', create: (context) => new AssetViewer(context) }
];

const sceneById = new Map(scenes.map((s) => [s.id, s]));

export function createContent(id: string, context: SceneContext): Scene {
    const entry = sceneById.get(id) ?? sceneById.get('')!;
    return entry.create(context);
}
