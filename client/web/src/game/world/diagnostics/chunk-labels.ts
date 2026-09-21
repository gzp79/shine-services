import * as THREE from 'three';
import type { DebugPanel } from '../../../engine/compositor/debug-panel';
import { EventSubscriptions } from '../../../engine/events';
import type { TextSpriteFactory } from '../../../engine/resources/text-sprite';
import type { Chunk } from '../chunk';
import type { GameWorld } from '../world';
import { ENTITY_LOADED, ENTITY_UNLOADED, type EntityLoadedEvent, type EntityUnloadedEvent } from '../world-events';

const LABEL_BACKGROUND = 'rgba(0, 0, 0, 0.7)';

/** Debug overlay: a "(q, r)" sprite centered on each loaded chunk. Event-driven — sprites change only on toggle or chunk load/unload. */
export class ChunkLabels {
    private readonly labels = new Map<string, THREE.Sprite>();
    private readonly subscriptions: EventSubscriptions;
    private readonly params = { show: false };

    constructor(
        private readonly world: GameWorld,
        private readonly textSprites: TextSpriteFactory,
        events: EventTarget,
        debugPanel: DebugPanel | null
    ) {
        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<EntityLoadedEvent>(ENTITY_LOADED, ({ id }) => {
            if (id.kind !== 'chunk' || !this.params.show) return;
            const chunk = this.world.getEntity(id);
            if (chunk) this.addLabel(chunk as Chunk);
        });
        this.subscriptions.on<EntityUnloadedEvent>(ENTITY_UNLOADED, ({ id }) => {
            if (id.kind === 'chunk') this.removeLabel(id.key());
        });

        debugPanel
            ?.scope('Controls')
            .add(this.params, 'show')
            .name('Show Chunk Labels')
            .onChange((show: boolean) => this.setShown(show));
    }

    private setShown(show: boolean): void {
        if (show) {
            for (const chunk of this.world.chunks.values()) this.addLabel(chunk);
        } else {
            this.clear();
        }
    }

    private addLabel(chunk: Chunk): void {
        const key = chunk.id.key();
        if (this.labels.has(key)) return;

        const sprite = this.textSprites.create(`(${chunk.id.q}, ${chunk.id.r})`, { background: LABEL_BACKGROUND });
        sprite.scale.set(200, 100, 1); // Scale in world units
        sprite.position.set(0, 0, 50); // Above the chunk center
        sprite.renderOrder = 998;
        chunk.group.add(sprite);
        this.labels.set(key, sprite);
    }

    private removeLabel(key: string): void {
        const sprite = this.labels.get(key);
        if (!sprite) return;
        sprite.removeFromParent();
        this.labels.delete(key);
    }

    private clear(): void {
        for (const sprite of this.labels.values()) sprite.removeFromParent();
        this.labels.clear();
    }

    /** Removes the label sprites; the shared factory is Game-owned and freed there. */
    dispose(): void {
        this.subscriptions.dispose();
        this.clear();
    }
}
