import * as THREE from 'three';

/**
 * Performance metrics display in bottom-right corner.
 * Tracks FPS, frame time, renderer stats, and memory usage.
 */
export class PerformanceMetrics {
    private readonly container: HTMLDivElement;
    private readonly fpsElement: HTMLDivElement;
    private readonly frameTimeElement: HTMLDivElement;
    private readonly drawCallsElement: HTMLDivElement;
    private readonly trianglesElement: HTMLDivElement;
    private readonly memoryElement: HTMLDivElement;

    private frameCount = 0;
    private lastFpsUpdate = 0;
    private fps = 0;
    private frameTimeSamples: number[] = [];
    private readonly maxSamples = 60;

    constructor(private readonly renderer: THREE.WebGLRenderer) {
        // Create container
        this.container = document.createElement('div');
        this.container.style.position = 'fixed';
        this.container.style.bottom = '10px';
        this.container.style.right = '10px';
        this.container.style.padding = '10px';
        this.container.style.backgroundColor = 'rgba(0, 0, 0, 0.7)';
        this.container.style.color = '#00ff00';
        this.container.style.fontFamily = 'monospace';
        this.container.style.fontSize = '12px';
        this.container.style.lineHeight = '1.4';
        this.container.style.borderRadius = '4px';
        this.container.style.pointerEvents = 'none';
        this.container.style.userSelect = 'none';
        this.container.style.zIndex = '1000';

        // Create metric elements
        this.fpsElement = this.createMetricElement('FPS', '0');
        this.frameTimeElement = this.createMetricElement('Frame', '0.0 ms');
        this.drawCallsElement = this.createMetricElement('Calls', '0');
        this.trianglesElement = this.createMetricElement('Tris', '0');
        this.memoryElement = this.createMetricElement('Memory', '0 MB');

        document.body.appendChild(this.container);
    }

    private createMetricElement(label: string, initialValue: string): HTMLDivElement {
        const line = document.createElement('div');
        line.innerHTML = `<span style="color: #888">${label}:</span> ${initialValue}`;
        this.container.appendChild(line);
        return line;
    }

    private updateMetricElement(element: HTMLDivElement, label: string, value: string): void {
        element.innerHTML = `<span style="color: #888">${label}:</span> ${value}`;
    }

    update(deltaTime: number): void {
        // Track frame time
        const frameTime = deltaTime * 1000; // Convert to ms
        this.frameTimeSamples.push(frameTime);
        if (this.frameTimeSamples.length > this.maxSamples) {
            this.frameTimeSamples.shift();
        }

        // Update FPS counter (every second)
        this.frameCount++;
        const now = performance.now();
        if (now - this.lastFpsUpdate >= 1000) {
            this.fps = Math.round((this.frameCount * 1000) / (now - this.lastFpsUpdate));
            this.frameCount = 0;
            this.lastFpsUpdate = now;
        }

        // Calculate average frame time
        const avgFrameTime = this.frameTimeSamples.reduce((a, b) => a + b, 0) / this.frameTimeSamples.length;

        // Get renderer info
        const info = this.renderer.info;
        const drawCalls = info.render.calls;
        const triangles = info.render.triangles;

        // Get memory usage (if available)
        const memory = (performance as any).memory;
        const memoryMB = memory ? Math.round(memory.usedJSHeapSize / 1048576) : 0;

        // Update display
        const fpsColor = this.fps >= 55 ? '#00ff00' : this.fps >= 30 ? '#ffff00' : '#ff0000';
        this.updateMetricElement(this.fpsElement, 'FPS', `<span style="color: ${fpsColor}">${this.fps}</span>`);
        this.updateMetricElement(this.frameTimeElement, 'Frame', `${avgFrameTime.toFixed(1)} ms`);
        this.updateMetricElement(this.drawCallsElement, 'Calls', `${drawCalls}`);
        this.updateMetricElement(this.trianglesElement, 'Tris', `${this.formatNumber(triangles)}`);

        if (memory) {
            this.updateMetricElement(this.memoryElement, 'Memory', `${memoryMB} MB`);
        }
    }

    private formatNumber(num: number): string {
        if (num >= 1000000) {
            return (num / 1000000).toFixed(1) + 'M';
        } else if (num >= 1000) {
            return (num / 1000).toFixed(1) + 'K';
        }
        return num.toString();
    }

    dispose(): void {
        this.container.remove();
    }
}
