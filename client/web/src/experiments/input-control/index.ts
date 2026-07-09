import { WebGPURenderer } from 'three/webgpu';
import { StrokeLineOverlay } from '../../engine/compositor/stroke-line-overlay';
import type { InputHandler, Point } from '../../engine/input/input-handler';
import { InputManager } from '../../engine/input/input-manager';
import { GestureSchema } from '../../engine/input/schemas/gesture-schema';
import { Experiment } from '../experiment';

export interface InputControlExperiment {
    dispose(): void;
}

interface EventLogEntry {
    timestamp: number;
    eventName: string;
    data: string;
}

class InputControl extends Experiment {
    private readonly inputManager: InputManager;
    private readonly strokeLine: StrokeLineOverlay;
    private readonly logDiv: HTMLDivElement;
    private readonly controllerDiv: HTMLDivElement;
    private readonly gestureSchema: GestureSchema;
    private readonly eventLog: EventLogEntry[] = [];
    private readonly maxLogEntries = 12;

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer, { addOrbitCamera: false });

        this.logDiv = document.createElement('div');
        this.logDiv.style.cssText = `
            position: absolute; top: 48px; left: 10px;
            background: rgba(0,0,0,0.8); color: #00ff00;
            font-family: monospace; font-size: 11px;
            padding: 8px; border-radius: 4px;
            pointer-events: none; line-height: 1.3;
        `;
        container.appendChild(this.logDiv);

        this.controllerDiv = document.createElement('div');
        this.controllerDiv.style.cssText = `
            position: absolute; top: 48px; right: 10px;
            background: rgba(0,0,0,0.8); color: #ffffff;
            font-family: monospace; font-size: 14px;
            padding: 15px; border-radius: 4px;
            pointer-events: none; font-weight: bold;
            white-space: pre;
        `;
        container.appendChild(this.controllerDiv);

        const fmtPoint = (p: Point) => `(${Math.round(p.x)}, ${Math.round(p.y)})`;

        const inputHandler: InputHandler = {
            onSchemaChanged: (s) => this.addLogEntry('schemaChanged', s),
            onPointerAt: (p) => this.addLogEntry('pointerAt', fmtPoint(p)),
            onPointerLeave: () => this.addLogEntry('pointerLeave', ''),
            onMoveTo: (p) => this.addLogEntry('moveTo', fmtPoint(p)),
            onRotateBy: (d) => this.addLogEntry('rotateBy', d.toFixed(2)),
            onZoomBy: (d) => this.addLogEntry('zoomBy', d.toFixed(1)),
            onMoveRate: (x, y, s) =>
                this.addLogEntry('moveRate', `x=${x.toFixed(2)} y=${y.toFixed(2)}${s ? ' sprint' : ''}`),
            onRotateRate: (v) => this.addLogEntry('rotateRate', v.toFixed(2)),
            onZoomRate: (v) => this.addLogEntry('zoomRate', v.toFixed(2)),
            onPinchStart: ([p1, p2]) => this.addLogEntry('pinchStart', `${fmtPoint(p1)} / ${fmtPoint(p2)}`),
            onPinch: ([s1, s2], [v1, v2], [c1, c2]) =>
                this.addLogEntry(
                    'pinch',
                    `start:${fmtPoint(s1)}/${fmtPoint(s2)} prev:${fmtPoint(v1)}/${fmtPoint(v2)} cur:${fmtPoint(c1)}/${fmtPoint(c2)}`
                ),
            onPinchEnd: ([s1, s2], [e1, e2]) =>
                this.addLogEntry(
                    'pinchEnd',
                    `start:${fmtPoint(s1)}/${fmtPoint(s2)} end:${fmtPoint(e1)}/${fmtPoint(e2)}`
                ),
            onInteractStart: (s) => this.addLogEntry('interactStart', fmtPoint(s)),
            onInteract: (s, p, c) =>
                this.addLogEntry('interact', `start:${fmtPoint(s)} prev:${fmtPoint(p)} cur:${fmtPoint(c)}`),
            onInteractEnd: (s, e) => this.addLogEntry('interactEnd', `start:${fmtPoint(s)} end:${fmtPoint(e)}`),
            onGesture: (pts) => this.addLogEntry('gesture', `${pts.length / 2} pts`)
        };

        this.inputManager = new InputManager(inputHandler, container);
        this.strokeLine = new StrokeLineOverlay(1000, 0x00ff00);
        this.gestureSchema = this.inputManager.schemas.find((s): s is GestureSchema => s instanceof GestureSchema)!;
        this.renderContext.addOverlay(this.strokeLine);

        this.start();
    }

    private addLogEntry(eventName: string, data: string) {
        this.eventLog.unshift({ timestamp: performance.now(), eventName, data });
        if (this.eventLog.length > this.maxLogEntries) this.eventLog.pop();
        this.updateLogDisplay();
    }

    private updateLogDisplay() {
        this.logDiv.innerHTML = this.eventLog
            .map((entry) => {
                const time = (entry.timestamp / 1000).toFixed(2);
                let color = '#00ff00';
                if (entry.eventName.includes('schema')) color = '#ff00ff';
                else if (entry.eventName.includes('moveTo')) color = '#00bfff';
                else if (entry.eventName.includes('rotate')) color = '#ffff00';
                else if (entry.eventName.includes('zoom')) color = '#ff1493';
                else if (entry.eventName.includes('Rate')) color = '#7fff00';
                else if (entry.eventName.includes('pinch')) color = '#ff8c00';
                else if (entry.eventName.includes('interact')) color = '#ff4500';
                return `<div style="color:${color}">[${time}s] ${entry.eventName} ${entry.data}</div>`;
            })
            .join('');
    }

    protected onUpdate(_deltaTime: number) {
        const schema = this.inputManager.activeSchema;
        if (!schema) {
            this.controllerDiv.textContent = 'Controller: none';
        } else {
            this.controllerDiv.textContent = `Controller: ${schema.name.toUpperCase()}\n${schema.state()}`;
        }

        const { buf, count } = this.gestureSchema.currentPoints;
        if (count > 0) {
            this.strokeLine.update(buf, count);
        } else {
            this.strokeLine.clear();
        }
    }

    dispose() {
        this.inputManager.dispose();
        this.strokeLine.dispose();
        this.logDiv.remove();
        this.controllerDiv.remove();
        super.dispose();
    }
}

export async function createInputControlExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer
): Promise<InputControlExperiment> {
    return new InputControl(container, renderer);
}
