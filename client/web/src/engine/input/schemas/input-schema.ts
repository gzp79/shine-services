import type { InputHandler } from '../input-handler';

export abstract class InputSchema {
    onActivated?: (schema: InputSchema) => void;

    private _handler?: InputHandler;
    private _enabled = true;

    constructor(
        readonly name: string,
        handler?: InputHandler
    ) {
        this._handler = handler;
    }

    get enabled(): boolean {
        return this._enabled;
    }
    set enabled(value: boolean) {
        if (this._enabled === value) return;
        this._enabled = value;
        if (!value) this.cancel();
        this.applyEnabled(value);
    }

    get handler(): InputHandler | undefined {
        return this._handler;
    }
    set handler(value: InputHandler | undefined) {
        if (this._handler === value) return;
        if (this._handler) this.cancel();
        this._handler = value;
    }

    get isActive(): boolean {
        return this._handler !== undefined;
    }

    protected activate(): boolean {
        if (!this.isActive) this.onActivated?.(this);
        return this.isActive;
    }

    abstract get isIdle(): boolean;
    abstract state(): Record<string, string>;
    abstract cancel(): void;
    abstract dispose(): void;

    // Fan the enabled state out to every raw input the schema owns.
    protected abstract applyEnabled(value: boolean): void;
}
