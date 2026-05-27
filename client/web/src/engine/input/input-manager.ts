import type { InputSchema } from './schemas/input-schema';
import type { InputHandler } from './input-handler';
import { DesktopSchema } from './schemas/desktop-schema';
import { TouchSchema } from './schemas/touch-schema';

export class InputManager {
    private readonly schemas: InputSchema[] = [];
    private lastUsedSchema: InputSchema | null = null;

    constructor(
        private readonly handler: InputHandler,
        container: HTMLElement
    ) {
        this.addSchema(new DesktopSchema(container));
        this.addSchema(new TouchSchema(container));
    }

    addSchema(schema: InputSchema): void {
        this.schemas.push(schema);

        schema.onMoveTo = (pos) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onMoveTo(pos);
        };
        schema.onRotateBy = (angleDelta) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onRotateBy(angleDelta);
        };
        schema.onZoomBy = (delta) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onZoomBy(delta);
        };

        schema.onMoveRate = (x, y, sprint) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onMoveRate(x, y, sprint);
        };
        schema.onRotateRate = (value) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onRotateRate(value);
        };
        schema.onZoomRate = (value) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onZoomRate(value);
        };

        schema.onPinchStart = (pos1, pos2) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onPinchStart(pos1, pos2);
        };
        schema.onPinch = (pos1, pos2) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onPinch(pos1, pos2);
        };
        schema.onPinchEnd = () => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onPinchEnd();
        };
        schema.onInteractStart = (pos) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onInteractStart(pos);
        };
        schema.onInteract = (pos) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onInteract(pos);
        };
        schema.onInteractEnd = (pos) => {
            if (!this.shouldAcceptCallback(schema)) return;
            this.handler.onInteractEnd(pos);
        };
    }

    private shouldAcceptCallback(schema: InputSchema): boolean {
        if (this.lastUsedSchema !== null) {
            return schema === this.lastUsedSchema;
        }
        return true;
    }

    private getActiveSchema(): InputSchema | null {
        for (const schema of this.schemas) {
            if (schema.isActive()) {
                if (this.lastUsedSchema !== schema) {
                    this.lastUsedSchema = schema;
                    const schemaType = schema instanceof DesktopSchema ? 'desktop' : 'touch';
                    this.handler.onSchemaChanged(schemaType);
                }
                return schema;
            }
        }

        if (this.lastUsedSchema !== null) {
            this.lastUsedSchema = null;
        }

        return null;
    }

    update(): void {
        // Poll active schema to track changes
        this.getActiveSchema();
    }

    dispose(): void {
        for (const schema of this.schemas) {
            schema.dispose();
        }
    }
}
