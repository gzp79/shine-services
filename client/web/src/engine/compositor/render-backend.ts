export class RendererUnavailableError extends Error {
    constructor() {
        super(
            'No graphics backend is available in this browser.\n' +
                'WebGPU and WebGL2 are both disabled or blocked.\n' +
                'Enable hardware acceleration, or try a different browser.'
        );
        this.name = 'RendererUnavailableError';
    }
}

// Narrow view of navigator.gpu — the DOM lib has no WebGPU types and it isn't worth a dependency.
interface GpuLike {
    requestAdapter(): Promise<unknown>;
}

// Mirrors Three's fallback order: prefer a real WebGPU adapter, else a WebGL2 context.
export async function hasRenderBackend(): Promise<boolean> {
    const gpu = (navigator as { gpu?: GpuLike }).gpu;
    if (gpu) {
        try {
            if (await gpu.requestAdapter()) return true;
        } catch {
            // Blocklisted or failed adapter request — fall through to the WebGL2 probe.
        }
    }
    return document.createElement('canvas').getContext('webgl2') !== null;
}
