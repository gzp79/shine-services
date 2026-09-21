import type { Selection } from './selection/selection';
import type { WorldEntityId } from './world-entity';

// Dispatched right after a world entity is added to the world group.
export const ENTITY_LOADED = 'entityloaded';
export type EntityLoadedEvent = {
    id: WorldEntityId;
};

// Dispatched right before a world entity is removed from the world group and disposed.
export const ENTITY_UNLOADED = 'entityunloaded';
export type EntityUnloadedEvent = {
    id: WorldEntityId;
};

// Dispatched when the active selection changes (null clears it).
export const SELECTION_CHANGED = 'selectionchanged';
export type SelectionChangedEvent = {
    selection: Selection | null;
};
