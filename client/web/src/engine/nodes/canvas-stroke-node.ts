/**
 * Draws an interleaved NDC [x0,y0,x1,y1,...] point path onto a Canvas2D overlay
 * positioned over the given container. NDC convention: center=(0,0), x right, y up, range [-1,1].
 *
 * Denormalizes to pixel coords at draw time, so the overlay is always correct
 * regardless of container resize.
 *
 * Screen-space only. No Three.js dependency.
 */
export class CanvasStrokeNode {
    private readonly canvas: HTMLCanvasElement;
    private readonly ctx: CanvasRenderingContext2D;

    constructor(
        private readonly container: HTMLElement,
        readonly color = '#ff4500',
        readonly lineWidth = 2
    ) {
        this.canvas = document.createElement('canvas');
        this.canvas.style.cssText = `
            position: absolute; inset: 0;
            pointer-events: none;
            width: 100%; height: 100%;
        `;
        this.container.appendChild(this.canvas);
        this.ctx = this.canvas.getContext('2d')!;
        this.syncSize();
    }

    private syncSize(): void {
        this.canvas.width = this.container.clientWidth;
        this.canvas.height = this.container.clientHeight;
    }

    /** buf: interleaved NDC [x,y,...], count: number of points. */
    draw(buf: Float32Array, count: number): void {
        this.syncSize();
        const w = this.canvas.width;
        const h = this.canvas.height;

        this.ctx.clearRect(0, 0, w, h);
        if (count < 2) return;

        // NDC → pixel: px = (x+1)/2 * w,  py = (1-y)/2 * h
        this.ctx.beginPath();
        this.ctx.moveTo(((buf[0] + 1) / 2) * w, ((1 - buf[1]) / 2) * h);
        for (let i = 1; i < count; i++) {
            this.ctx.lineTo(((buf[i * 2] + 1) / 2) * w, ((1 - buf[i * 2 + 1]) / 2) * h);
        }
        this.ctx.strokeStyle = this.color;
        this.ctx.lineWidth = this.lineWidth;
        this.ctx.lineJoin = 'round';
        this.ctx.lineCap = 'round';
        this.ctx.stroke();
    }

    clear(): void {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    }

    dispose(): void {
        this.canvas.remove();
    }
}
