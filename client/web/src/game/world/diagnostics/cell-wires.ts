import type { DebugPanel } from '../../../engine/compositor/debug-panel';
import { EventSubscriptions } from '../../../engine/events';
import { WireMesh } from '../../../engine/scene/wire-mesh';
import { type PolygonMeshLike, asPolygonMesh } from '../../../mesh/polygon-mesh';
import type { Chunk } from '../chunk';
import type { ChunkCorner } from '../chunk-corner';
import type { ChunkEdge } from '../chunk-edge';
import type { GameWorld } from '../world';
import type { WorldEntity } from '../world-entity';
import { ENTITY_LOADED, ENTITY_UNLOADED, type EntityLoadedEvent, type EntityUnloadedEvent } from '../world-events';

/** Cell-boundary polygons to wire for an entity, or null for kinds that have none. */
function entityPolygons(entity: WorldEntity): PolygonMeshLike | null {
    switch (entity.id.kind) {
        case 'chunk':
            return asPolygonMesh((entity as Chunk).cells);
        case 'edge':
            return asPolygonMesh((entity as ChunkEdge).cells);
        case 'corner':
            return asPolygonMesh((entity as ChunkCorner).cells);
    }
}

/** Debug overlay: a cell-boundary wireframe over every world entity. Event-driven — wires change only on toggle or entity load/unload. */
export class CellWires {
    private readonly wires = new Map<string, WireMesh>();
    private readonly subscriptions: EventSubscriptions;
    private readonly params = { show: false };

    constructor(
        private readonly world: GameWorld,
        events: EventTarget,
        debugPanel: DebugPanel | null
    ) {
        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<EntityLoadedEvent>(ENTITY_LOADED, ({ id }) => {
            if (!this.params.show) return;
            const entity = this.world.getEntity(id);
            if (entity) this.addWire(entity);
        });
        this.subscriptions.on<EntityUnloadedEvent>(ENTITY_UNLOADED, ({ id }) => this.removeWire(id.key()));

        debugPanel
            ?.scope('Controls')
            .add(this.params, 'show')
            .name('Show Cell Wires')
            .onChange((show: boolean) => this.setShown(show));
    }

    private setShown(show: boolean): void {
        if (show) {
            for (const entity of this.world.entities()) this.addWire(entity);
        } else {
            this.clear();
        }
    }

    private addWire(entity: WorldEntity): void {
        const key = entity.id.key();
        if (this.wires.has(key)) return;

        const polygons = entityPolygons(entity);
        if (!polygons) return;

        const wire = WireMesh.fromPolygons(entity.group, polygons);
        wire.show();
        this.wires.set(key, wire);
    }

    private removeWire(key: string): void {
        const wire = this.wires.get(key);
        if (!wire) return;
        wire.dispose();
        this.wires.delete(key);
    }

    private clear(): void {
        for (const wire of this.wires.values()) wire.dispose();
        this.wires.clear();
    }

    dispose(): void {
        this.subscriptions.dispose();
        this.clear();
    }
}
