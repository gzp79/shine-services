import { WebGPURenderer } from 'three/webgpu';
import { InputManager } from '../../engine/input/input-manager';
import type { InputHandler, Point } from '../../engine/input/input-handler';
import { animate, createExperiment, type ExperimentContext } from '../experiment';

export interface InputControlExperiment {
    dispose(): void;
}

interface EventLogEntry {
    timestamp: number;
    eventName: string;
    data: string;
}

export async function createInputControlExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer
): Promise<InputControlExperiment> {
    const ctx: ExperimentContext = createExperiment(container, renderer, { addOrbitCamera: false });

    const eventLog: EventLogEntry[] = [];
    const maxLogEntries = 12;

    const logDiv = document.createElement('div');
    logDiv.style.cssText = `
        position: absolute; top: 48px; left: 10px;
        background: rgba(0,0,0,0.8); color: #00ff00;
        font-family: monospace; font-size: 11px;
        padding: 8px; border-radius: 4px;
        pointer-events: none; line-height: 1.3;
    `;
    container.appendChild(logDiv);

    const controllerDiv = document.createElement('div');
    controllerDiv.style.cssText = `
        position: absolute; top: 48px; right: 10px;
        background: rgba(0,0,0,0.8); color: #ffffff;
        font-family: monospace; font-size: 14px;
        padding: 15px; border-radius: 4px;
        pointer-events: none; font-weight: bold;
        white-space: pre;
    `;
    container.appendChild(controllerDiv);

    function addLogEntry(eventName: string, data: string) {
        eventLog.unshift({ timestamp: performance.now(), eventName, data });
        if (eventLog.length > maxLogEntries) eventLog.pop();
        updateLogDisplay();
    }

    function fmtPoint(p: Point) { return `(${Math.round(p.x)}, ${Math.round(p.y)})`; }

    function updateLogDisplay() {
        logDiv.innerHTML = eventLog.map((entry) => {
            const time = (entry.timestamp / 1000).toFixed(2);
            let color = '#00ff00';
            if (entry.eventName.includes('schema'))       color = '#ff00ff';
            else if (entry.eventName.includes('moveTo'))  color = '#00bfff';
            else if (entry.eventName.includes('rotate'))  color = '#ffff00';
            else if (entry.eventName.includes('zoom'))    color = '#ff1493';
            else if (entry.eventName.includes('Rate'))    color = '#7fff00';
            else if (entry.eventName.includes('pinch'))   color = '#ff8c00';
            else if (entry.eventName.includes('interact'))color = '#ff4500';
            return `<div style="color:${color}">[${time}s] ${entry.eventName} ${entry.data}</div>`;
        }).join('');
    }

    const inputHandler: InputHandler = {
        onSchemaChanged: (s) => {
            addLogEntry('schemaChanged', s);
        },

        onMoveTo:    (p) => addLogEntry('moveTo',    fmtPoint(p)),
        onRotateBy:  (d) => addLogEntry('rotateBy',  `${(d * 180 / Math.PI).toFixed(1)}°`),
        onZoomBy:    (d) => addLogEntry('zoomBy',    d.toFixed(1)),

        onMoveRate:   (x, y, s) => addLogEntry('moveRate',   `x=${x.toFixed(2)} y=${y.toFixed(2)}${s ? ' sprint' : ''}`),
        onRotateRate: (v)       => addLogEntry('rotateRate', v.toFixed(2)),
        onZoomRate:   (v)       => addLogEntry('zoomRate',   v.toFixed(2)),

        onPinchStart: ([p1, p2])                    => addLogEntry('pinchStart', `${fmtPoint(p1)} / ${fmtPoint(p2)}`),
        onPinch:      ([s1, s2], [v1, v2], [c1, c2]) => addLogEntry('pinch', `start:${fmtPoint(s1)}/${fmtPoint(s2)} prev:${fmtPoint(v1)}/${fmtPoint(v2)} cur:${fmtPoint(c1)}/${fmtPoint(c2)}`),
        onPinchEnd:   ([s1, s2], [e1, e2])           => addLogEntry('pinchEnd', `start:${fmtPoint(s1)}/${fmtPoint(s2)} end:${fmtPoint(e1)}/${fmtPoint(e2)}`),

        onInteractStart: (s)       => addLogEntry('interactStart', fmtPoint(s)),
        onInteract:      (s, p, c) => addLogEntry('interact',      `start:${fmtPoint(s)} prev:${fmtPoint(p)} cur:${fmtPoint(c)}`),
        onInteractEnd:   (s, e)    => addLogEntry('interactEnd',   `start:${fmtPoint(s)} end:${fmtPoint(e)}`),
    };

    const inputManager = new InputManager(inputHandler, container);

    function updateControllerDisplay() {
        const schema = inputManager.activeSchema;
        if (!schema) {
            controllerDiv.textContent = 'Controller: none';
            return;
        }
        controllerDiv.textContent = `Controller: ${schema.name.toUpperCase()}\n${schema.state()}`;
    }

    const stopAnimation = animate(ctx, updateControllerDisplay);

    return {
        dispose() {
            stopAnimation();
            inputManager.dispose();
            logDiv.remove();
            controllerDiv.remove();
            ctx.resizeObserver.disconnect();
        }
    };
}
