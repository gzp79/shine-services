import type { DebugPanel } from '../../../engine/compositor/debug-panel';
import { EventDispatcher } from '../../../engine/events';
import { SELECTION_CHANGED, type Selection, type SelectionChangedEvent } from './selection-event';

const SCOPE = 'World';

export class SelectionManager {
    private _current: Selection | null = null;
    private readonly dispatcher: EventDispatcher;
    private readonly debugPanel: DebugPanel | null;

    constructor(events: EventTarget, debugPanel: DebugPanel | null) {
        this.dispatcher = new EventDispatcher(events);
        this.debugPanel = debugPanel;
    }

    get current(): Selection | null {
        return this._current;
    }

    set(selection: Selection): void {
        if (this.isSame(selection)) return;
        this._current = selection;
        this.dispatcher.dispatch<SelectionChangedEvent>(SELECTION_CHANGED, { selection });
        this.updateDebugPanel(selection);
    }

    clearIfOwner(owner: object): void {
        const cur = this._current;
        if (!cur) return;
        const curOwner = cur.type === 'cell' ? cur.chunk : cur.type === 'edge-cell' ? cur.edge : cur.corner;
        if (curOwner === owner) this.clear();
    }

    clear(): void {
        if (!this._current) return;
        this._current = null;
        this.dispatcher.dispatch<SelectionChangedEvent>(SELECTION_CHANGED, { selection: null });
        this.debugPanel?.set(SCOPE, 'Selection', 'None');
    }

    private updateDebugPanel(sel: Selection): void {
        switch (sel.type) {
            case 'cell':
                this.debugPanel?.set(
                    SCOPE,
                    'Selection',
                    `[inner] (${sel.chunk.id.q}, ${sel.chunk.id.r})/${sel.cellId}`
                );
                break;
            case 'edge-cell':
                this.debugPanel?.set(
                    SCOPE,
                    'Selection',
                    `[edge] (${sel.edge.id.chunkId.q}, ${sel.edge.id.chunkId.r})-${sel.edge.id.edgeIdx}/${sel.cellId}`
                );
                break;
            case 'corner-cell':
                this.debugPanel?.set(
                    SCOPE,
                    'Selection',
                    `[corner] (${sel.corner.id.chunkId.q}, ${sel.corner.id.chunkId.r})-${sel.corner.id.cornerIdx}/${sel.cellId}`
                );
                break;
        }
    }

    private isSame(next: Selection): boolean {
        const cur = this._current;
        if (!cur || cur.cellId !== next.cellId) return false;
        switch (next.type) {
            case 'cell':
                return cur.type === 'cell' && cur.chunk === next.chunk;
            case 'edge-cell':
                return cur.type === 'edge-cell' && cur.edge === next.edge;
            case 'corner-cell':
                return cur.type === 'corner-cell' && cur.corner === next.corner;
        }
    }
}
