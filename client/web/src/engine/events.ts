/**
 * Dispatches typed events.
 */
export class EventDispatcher {
    constructor(readonly events: EventTarget) {}

    /**
     * Dispatch a typed event.
     * Example: dispatcher.dispatch(WORLD_REFERENCE_CHANGED, { oldChunkId, newChunkId, ... })
     */
    dispatch<T>(eventName: string, detail: T): void {
        this.events.dispatchEvent(new CustomEvent(eventName, { detail }));
    }
}

/**
 * Manages event subscriptions with automatic cleanup.
 */
export class EventSubscriptions {
    private readonly named = new Map<string, () => void>();

    constructor(readonly events: EventTarget) {}

    /**
     * Subscribe to a typed event with automatic cleanup.
     * Only one listener per event name - calling again replaces the previous.
     * Example: subscriptions.on<ViewportResizeEvent>(VIEWPORT_RESIZE, (event) => { ... })
     */
    on<T>(eventName: string, handler: (event: T) => void): void {
        this.remove(eventName);

        const listener = (e: Event) => {
            const customEvent = e as CustomEvent<T>;
            handler(customEvent.detail);
        };
        this.events.addEventListener(eventName, listener);
        this.named.set(eventName, () => this.events.removeEventListener(eventName, listener));
    }

    /**
     * Subscribe to a window event with automatic cleanup.
     * Only one listener per event name - calling again replaces the previous.
     * Example: subscriptions.listenWindow('keydown', (e) => { ... })
     */
    listenWindow<K extends keyof WindowEventMap>(eventName: K, handler: (event: WindowEventMap[K]) => void): void {
        this.remove(eventName);

        const listener = handler as EventListener;
        window.addEventListener(eventName, listener);
        this.named.set(eventName, () => window.removeEventListener(eventName, listener));
    }

    /**
     * Remove a subscription by name.
     * Example: subscriptions.remove('keydown')
     */
    remove(eventName: string): void {
        const cleanup = this.named.get(eventName);
        if (cleanup) {
            cleanup();
            this.named.delete(eventName);
        }
    }

    /**
     * Remove all event subscriptions. Call in your destroy() method.
     */
    destroy(): void {
        for (const cleanup of this.named.values()) {
            cleanup();
        }
        this.named.clear();
    }
}
