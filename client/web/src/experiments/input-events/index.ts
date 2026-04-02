import * as THREE from 'three';
import { EventSubscriptions } from '../../engine/events';
import {
    INPUT_CONTROLLER_CHANGED,
    INPUT_INTERACT_DRAG,
    INPUT_INTERACT_END,
    INPUT_INTERACT_START,
    INPUT_MOVE,
    INPUT_PAN,
    INPUT_PAN_END,
    INPUT_PAN_START,
    INPUT_PINCH,
    INPUT_PINCH_END,
    INPUT_PINCH_START,
    INPUT_ROTATE,
    INPUT_ROTATE_END,
    INPUT_ROTATE_START,
    INPUT_TAP,
    INPUT_ZOOM,
    InputController,
    type InputControllerChangedEvent,
    type InputInteractDragEvent,
    type InputInteractEndEvent,
    type InputInteractStartEvent,
    type InputMoveEvent,
    type InputPanEndEvent,
    type InputPanEvent,
    type InputPanStartEvent,
    type InputPinchEndEvent,
    type InputPinchEvent,
    type InputPinchStartEvent,
    type InputRotateEndEvent,
    type InputRotateEvent,
    type InputRotateStartEvent,
    type InputTapEvent,
    type InputZoomEvent
} from '../../engine/input';
import { ExperimentContext, animate, createExperiment } from '../experiment';

export interface InputEventsViewer {
    dispose(): void;
}

interface EventLogEntry {
    timestamp: number;
    eventName: string;
    data: string;
}

export async function createInputEventsViewer(container: HTMLElement): Promise<InputEventsViewer> {
    const ctx: ExperimentContext = createExperiment(container);
    const events = new EventTarget();
    const subscriptions = new EventSubscriptions(events);

    // Create InputController
    const inputController = new InputController(ctx.renderer.domElement, events);

    // Add a simple plane to the scene
    const planeGeometry = new THREE.PlaneGeometry(4, 4);
    const planeMaterial = new THREE.MeshStandardMaterial({ color: 0x4a4a6a });
    const plane = new THREE.Mesh(planeGeometry, planeMaterial);
    ctx.scene.add(plane);

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
            const { direction } = obj as InputMoveEvent;
            return `dir=(${direction.x.toFixed(2)}, ${direction.y.toFixed(2)})`;
        }
        if (eventName.includes('TAP') || eventName.includes('_START') || eventName.includes('_END')) {
            const pos = obj.pos || obj;
            return `pos=(${Math.round(pos.x)}, ${Math.round(pos.y)})`;
        }
        if (eventName.includes('ZOOM')) {
            return `Δ=${obj.delta.toFixed(1)} pos=(${Math.round(obj.pos.x)}, ${Math.round(obj.pos.y)})`;
        }
        if (eventName.includes('PAN') || eventName.includes('ROTATE') || eventName.includes('INTERACT_DRAG')) {
            const { start, current } = obj;
            return `Δ=(${Math.round(current.x - start.x)}, ${Math.round(current.y - start.y)})`;
        }
        if (eventName.includes('PINCH')) {
            const [p1, p2] = obj.start;
            const [c1, c2] = obj.current;
            const startDist = Math.hypot(p2.x - p1.x, p2.y - p1.y);
            const currentDist = Math.hypot(c2.x - c1.x, c2.y - c1.y);
            return `dist=${Math.round(currentDist)} (${Math.round(startDist)})`;
        }
        if (eventName.includes('CONTROLLER_CHANGED')) {
            return obj.controller;
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
            else if (entry.eventName.includes('WASD'))
                color = '#7fff00'; // chartreuse
            else if (entry.eventName.includes('INTERACT')) color = '#ff4500'; // red-orange

            const formatted = formatEventData(entry.eventName, entry.data);
            return `<div style="color: ${color}">[${time}s] ${entry.eventName} ${formatted}</div>`;
        });
        logDiv.innerHTML = lines.join('');
    }

    // Subscribe to all input events
    // Controller changes
    subscriptions.on<InputControllerChangedEvent>(INPUT_CONTROLLER_CHANGED, (event) => {
        addLogEntry('CONTROLLER_CHANGED', event);
        controllerDiv.textContent = `Controller: ${event.controller.toUpperCase()}`;
        controllerDiv.style.color = event.controller === 'touch' ? '#00bfff' : '#ffffff';
    });

    subscriptions.on<InputInteractStartEvent>(INPUT_INTERACT_START, (event) => {
        addLogEntry('INTERACT_START', event);
    });

    subscriptions.on<InputInteractDragEvent>(INPUT_INTERACT_DRAG, (event) => {
        addLogEntry('INTERACT_DRAG', event);
    });

    subscriptions.on<InputInteractEndEvent>(INPUT_INTERACT_END, (event) => {
        addLogEntry('INTERACT_END', event);
    });

    subscriptions.on<InputTapEvent>(INPUT_TAP, (event) => {
        addLogEntry('TAP', event);
    });

    subscriptions.on<InputPanStartEvent>(INPUT_PAN_START, (event) => {
        addLogEntry('PAN_START', event);
    });

    subscriptions.on<InputPanEvent>(INPUT_PAN, (event) => {
        addLogEntry('PAN', event);
    });

    subscriptions.on<InputPanEndEvent>(INPUT_PAN_END, (event) => {
        addLogEntry('PAN_END', event);
    });

    subscriptions.on<InputRotateStartEvent>(INPUT_ROTATE_START, (event) => {
        addLogEntry('ROTATE_START', event);
    });

    subscriptions.on<InputRotateEvent>(INPUT_ROTATE, (event) => {
        addLogEntry('ROTATE', event);
    });

    subscriptions.on<InputRotateEndEvent>(INPUT_ROTATE_END, (event) => {
        addLogEntry('ROTATE_END', event);
    });

    subscriptions.on<InputZoomEvent>(INPUT_ZOOM, (event) => {
        addLogEntry('ZOOM', event);
    });

    subscriptions.on<InputMoveEvent>(INPUT_MOVE, (event) => {
        addLogEntry('MOVE', event);
    });

    subscriptions.on<InputPinchStartEvent>(INPUT_PINCH_START, (event) => {
        addLogEntry('PINCH_START', event);
    });

    subscriptions.on<InputPinchEvent>(INPUT_PINCH, (event) => {
        addLogEntry('PINCH', event);
    });

    subscriptions.on<InputPinchEndEvent>(INPUT_PINCH_END, (event) => {
        addLogEntry('PINCH_END', event);
    });

    const animationId = animate(ctx);

    return {
        dispose() {
            cancelAnimationFrame(animationId);
            inputController.dispose();
            subscriptions.dispose();
            logDiv.remove();
            controllerDiv.remove();
            ctx.resizeObserver.disconnect();
            ctx.renderer.dispose();
            ctx.renderer.domElement.remove();
        }
    };
}
