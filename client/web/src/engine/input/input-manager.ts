import { InputSchema } from './schemas/input-schema';
import type { InputHandler } from './input-handler';
import { DebugSchema } from './schemas/debug-schema';
import { DesktopSchema } from './schemas/desktop-schema';
import { TouchSchema } from './schemas/touch-schema';

export class InputManager {
    private readonly schemas: InputSchema[] = [];
    private _activeSchema: InputSchema | null = null;

    constructor(
        private readonly handler: InputHandler,
        container: HTMLElement
    ) {
        this.schemas.push(new DebugSchema());
        this.schemas.push(new DesktopSchema(container));
        this.schemas.push(new TouchSchema(container));

        for (const schema of this.schemas) {
            schema.onActivated = (s) => this.onActivated(s);
        }
    }

    get activeSchema(): InputSchema | null { return this._activeSchema; }

    private onActivated(schema: InputSchema): void {
        if (this._activeSchema && !this._activeSchema.isIdle) return;

        const prev = this._activeSchema;
        this._activeSchema = schema;

        if (prev) {
            prev.handler = undefined;
            this.handler.onSchemaChanged(schema.name);
        }

        this._activeSchema!.handler = this.handler;
    }

    dispose(): void {
        for (const schema of this.schemas) {
            schema.dispose();
        }
    }
}
