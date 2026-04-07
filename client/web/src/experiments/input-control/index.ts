import { InputController, type InputHandler } from '../../engine/input';
import type { Delta, Point } from '../../engine/input';
import { ExperimentContext, animate, createExperiment } from '../experiment';

export interface InputControlExperiment {
    dispose(): void;
}

interface EventLogEntry {
    timestamp: number;
    eventName: string;
    data: string;
}

export async function createInputControlExperiment(container: HTMLElement): Promise<InputControlExperiment> {
    const ctx: ExperimentContext = await createExperiment(container, { addOrbitCamera: false });

    // Event log (max 12 entries, compact display)
    const eventLog: EventLogEntry[] = [];
    const maxLogEntries = 12;

    // Create log display div
    const logDiv = document.createElement('div');
    logDiv.style.position = 'absolute';
    logDiv.style.top = '10px';
    logDiv.style.left = '10px';
    logDiv.style.backgroundColor = 'rgba(0, 0, 0, 0.8)';
    logDiv.style.color = '#00ff00';
    logDiv.style.fontFamily = 'monospace';
    logDiv.style.fontSize = '11px';
    logDiv.style.padding = '8px';
    logDiv.style.borderRadius = '4px';
    logDiv.style.pointerEvents = 'none';
    logDiv.style.lineHeight = '1.3';
    container.appendChild(logDiv);

    // Create controller display
    const controllerDiv = document.createElement('div');
    controllerDiv.style.position = 'absolute';
    controllerDiv.style.top = '10px';
    controllerDiv.style.right = '10px';
    controllerDiv.style.backgroundColor = 'rgba(0, 0, 0, 0.8)';
    controllerDiv.style.color = '#ffffff';
    controllerDiv.style.fontFamily = 'monospace';
    controllerDiv.style.fontSize = '16px';
    controllerDiv.style.padding = '15px';
    controllerDiv.style.borderRadius = '4px';
    controllerDiv.style.pointerEvents = 'none';
    controllerDiv.style.fontWeight = 'bold';
    controllerDiv.textContent = 'Controller: DESKTOP';
    container.appendChild(controllerDiv);

    function addLogEntry(eventName: string, data: unknown) {
        const entry: EventLogEntry = {
            timestamp: performance.now(),
            eventName,
            data: JSON.stringify(data, null, 2)
        };
        eventLog.unshift(entry);
        if (eventLog.length > maxLogEntries) {
            eventLog.pop();
        }
        updateLogDisplay();
    }

    function formatEventData(eventName: string, data: unknown): string {
        const obj = typeof data === 'string' ? JSON.parse(data) : data;

        if (eventName.includes('MOVE')) {
            const { direction, isSprinting } = obj as { direction: Delta; isSprinting: boolean };
            return `dir=(${direction.x.toFixed(2)}, ${direction.y.toFixed(2)}, ${isSprinting ? 'sprint' : 'walk'})`;
        }
        if (eventName.includes('TAP') || eventName.includes('_START') || eventName.includes('_END')) {
            const pos = obj as Point;
            return `pos=(${Math.round(pos.x)}, ${Math.round(pos.y)})`;
        }
        if (eventName.includes('ZOOM')) {
            const { pos, delta } = obj as { pos: Point; delta: number };
            return `Δ=${delta.toFixed(1)} pos=(${Math.round(pos.x)}, ${Math.round(pos.y)})`;
        }
        if (eventName.includes('PAN') || eventName.includes('ROTATE') || eventName.includes('INTERACT_DRAG')) {
            const { start, current } = obj as { start: Point; current: Point };
            return `Δ=(${Math.round(current.x - start.x)}, ${Math.round(current.y - start.y)})`;
        }
        if (eventName.includes('PINCH')) {
            const { start, current } = obj as { start: [Point, Point]; current: [Point, Point] };
            const [p1, p2] = start;
            const [c1, c2] = current;
            const startDist = Math.hypot(p2.x - p1.x, p2.y - p1.y);
            const currentDist = Math.hypot(c2.x - c1.x, c2.y - c1.y);
            return `dist=${Math.round(currentDist)} (${Math.round(startDist)})`;
        }
        if (eventName.includes('CONTROLLER_CHANGED')) {
            return obj as string;
        }
        return '';
    }

    function updateLogDisplay() {
        const lines = eventLog.map((entry) => {
            const time = (entry.timestamp / 1000).toFixed(2);

            // Color code event types
            let color = '#00ff00'; // default green
            if (entry.eventName.includes('CONTROLLER_CHANGED'))
                color = '#ff00ff'; // magenta
            else if (entry.eventName.includes('TAP'))
                color = '#00ff00'; // green
            else if (entry.eventName.includes('PAN'))
                color = '#00bfff'; // blue
            else if (entry.eventName.includes('ROTATE'))
                color = '#ffff00'; // yellow
            else if (entry.eventName.includes('PINCH'))
                color = '#ff8c00'; // orange
            else if (entry.eventName.includes('ZOOM'))
                color = '#ff1493'; // pink
            else if (entry.eventName.includes('MOVE'))
                color = '#7fff00'; // chartreuse
            else if (entry.eventName.includes('INTERACT')) color = '#ff4500'; // red-orange

            const formatted = formatEventData(entry.eventName, entry.data);
            return `<div style="color: ${color}">[${time}s] ${entry.eventName} ${formatted}</div>`;
        });
        logDiv.innerHTML = lines.join('');
    }

    // Create input handler that logs events directly
    const inputHandler: InputHandler = {
        onControllerChanged: (controller) => {
            addLogEntry('CONTROLLER_CHANGED', controller);
            controllerDiv.textContent = `Controller: ${controller.toUpperCase()}`;
            controllerDiv.style.color = controller === 'touch' ? '#00bfff' : '#ffffff';
        },
        onTap: (pos) => addLogEntry('TAP', pos),
        onInteractStart: (pos) => addLogEntry('INTERACT_START', pos),
        onInteractDrag: (start, current) => addLogEntry('INTERACT_DRAG', { start, current }),
        onInteractEnd: (pos) => addLogEntry('INTERACT_END', pos),
        onPanStart: (pos) => addLogEntry('PAN_START', pos),
        onPan: (start, current) => addLogEntry('PAN', { start, current }),
        onPanEnd: (pos) => addLogEntry('PAN_END', pos),
        onRotateStart: (pos) => addLogEntry('ROTATE_START', pos),
        onRotate: (start, current) => addLogEntry('ROTATE', { start, current }),
        onRotateEnd: (pos) => addLogEntry('ROTATE_END', pos),
        onPinchStart: (start, current) => addLogEntry('PINCH_START', { start, current }),
        onPinch: (start, current) => addLogEntry('PINCH', { start, current }),
        onPinchEnd: (start, current) => addLogEntry('PINCH_END', { start, current }),
        onZoom: (pos, delta) => addLogEntry('ZOOM', { pos, delta }),
        onMove: (direction, isSprinting) => addLogEntry('MOVE', { direction, isSprinting })
    };
    const inputController = new InputController(ctx.renderer.domElement, inputHandler);

    const animationId = animate(ctx);

    return {
        dispose() {
            cancelAnimationFrame(animationId);
            inputController.dispose();
            logDiv.remove();
            controllerDiv.remove();
            ctx.resizeObserver.disconnect();
            ctx.renderer.dispose();
            ctx.renderer.domElement.remove();
        }
    };
}
