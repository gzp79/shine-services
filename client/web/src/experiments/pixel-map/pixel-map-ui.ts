import type { ConnectionState } from '../../engine/net/stream-connection';
import { PALETTE_SIZE, paletteRgba } from './pixel-map-layout';

export type StatusTone = 'ok' | 'wait' | 'error';

export function statusText(
    connection: ConnectionState,
    stream: 'waiting' | 'live' | 'unsynced',
    version: number,
    pending: number
): { text: string; tone: StatusTone } {
    if (connection === 'closed') return { text: 'Disconnected', tone: 'error' };
    if (connection === 'replaced') return { text: 'Opened in another tab, reload to take over', tone: 'error' };
    if (connection === 'reconnecting') return { text: 'Reconnecting…', tone: 'wait' };
    if (stream === 'unsynced') return { text: 'Stream unavailable, retrying', tone: 'error' };
    if (connection === 'connecting' || stream === 'waiting') return { text: 'Connecting…', tone: 'wait' };
    const unconfirmed = pending > 0 ? ` · ${pending} unconfirmed` : '';
    return { text: `Live · v${version}${unconfirmed}`, tone: 'ok' };
}

export const SCATTER_COUNTS = [10, 20, 50, 100] as const;

const TONE_COLOR: Record<StatusTone, string> = { ok: '#6ee7a0', wait: '#ffd66b', error: '#ff7b7b' };

// The status chip and the palette swatches over the canvas. The chip never takes pointer events.
export class PixelMapUi {
    private readonly root: HTMLDivElement;
    private readonly chip: HTMLDivElement;
    private readonly user: HTMLDivElement;
    private readonly swatches: HTMLButtonElement[] = [];
    private selected = 1;

    constructor(host: HTMLElement, onScatter: (count: number) => void, onResync: () => void) {
        this.root = document.createElement('div');
        this.root.style.cssText = 'position:absolute;left:10px;top:10px;font:13px sans-serif;pointer-events:none;';
        this.chip = document.createElement('div');
        this.chip.style.cssText = 'padding:4px 10px;border-radius:12px;background:#000a;color:#fff;margin-bottom:6px;';
        this.root.append(this.chip);
        this.user = document.createElement('div');
        this.user.style.cssText = 'font-size:11px;color:#fffc;margin:0 0 6px 10px;text-shadow:0 0 3px #000;';
        this.root.append(this.user);

        const palette = document.createElement('div');
        palette.style.cssText = 'display:grid;grid-template-columns:repeat(8,22px);gap:3px;pointer-events:auto;';
        for (let index = 0; index < PALETTE_SIZE; index++) {
            const swatch = document.createElement('button');
            const [r, g, b] = paletteRgba(index);
            swatch.style.cssText = `width:22px;height:22px;border:2px solid transparent;border-radius:4px;background:rgb(${r},${g},${b});cursor:pointer;`;
            swatch.title = index === 0 ? 'Erase' : `Color ${index}`;
            swatch.addEventListener('click', () => this.select(index));
            palette.append(swatch);
            this.swatches.push(swatch);
        }
        this.root.append(palette);

        const scatter = document.createElement('div');
        scatter.style.cssText = 'display:flex;gap:4px;margin-top:6px;pointer-events:auto;';
        for (const count of SCATTER_COUNTS) {
            const button = document.createElement('button');
            button.textContent = `+${count}`;
            button.title = `Paint ${count} random pixels`;
            button.style.cssText = 'padding:2px 8px;border-radius:4px;border:0;cursor:pointer;';
            button.addEventListener('click', () => onScatter(count));
            scatter.append(button);
        }
        const resync = document.createElement('button');
        resync.textContent = 'Resync';
        resync.title = 'Ask the server for the state again, as after a connect';
        resync.style.cssText = 'padding:2px 8px;border-radius:4px;border:0;cursor:pointer;margin-left:auto;';
        resync.addEventListener('click', onResync);
        scatter.append(resync);
        this.root.append(scatter);
        host.append(this.root);
        this.select(this.selected);
    }

    get selectedColor(): number {
        return this.selected;
    }

    setStatus(text: string, tone: StatusTone): void {
        if (this.chip.textContent === text) return;
        this.chip.textContent = text;
        this.chip.style.borderLeft = `4px solid ${TONE_COLOR[tone]}`;
    }

    // Who this client is, when it knows.
    setUser(userId: string | null): void {
        this.user.textContent = userId ? `user ${userId.slice(0, 8)}` : '';
        this.user.title = userId ?? '';
    }

    dispose(): void {
        this.root.remove();
    }

    private select(index: number): void {
        this.selected = index;
        this.swatches.forEach((swatch, i) => {
            swatch.style.borderColor = i === index ? '#fff' : 'transparent';
        });
    }
}
