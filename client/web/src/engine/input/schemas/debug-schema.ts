import type { InputHandler } from '../input-handler';
import { InputSchema } from './input-schema';
import { RawKeyDown } from '../raw/raw-key-down';

/**
 * DebugSchema toggles on/off with Tab.
 */
export class DebugSchema extends InputSchema {
    private readonly tab: RawKeyDown;
    private _isIdle = true;

    constructor(handler?: InputHandler) {
        super('debug', handler);

        this.tab = new RawKeyDown('Tab');
        this.tab.onDown = () => {
            if (this.activate()) this._isIdle = !this._isIdle;
        };
    }

    get isIdle(): boolean { return this._isIdle; }

    state(): string {
        return `idle: ${this._isIdle}`;
    }

    cancel(): void {
        this._isIdle = true;
    }

    dispose(): void {
        this.tab.dispose();
    }
}
