import * as THREE from 'three';

export type TextSpriteStyle = {
    /** CSS font shorthand for the text. */
    font?: string;
    /** Text fill color. */
    color?: string;
    /** Background fill; transparent when omitted. */
    background?: string;
    /** Backing canvas size in pixels, which also sets the texture resolution. */
    canvasWidth?: number;
    canvasHeight?: number;
};

const DEFAULT_STYLE: Required<Omit<TextSpriteStyle, 'background'>> = {
    font: 'bold 48px monospace',
    color: 'white',
    canvasWidth: 256,
    canvasHeight: 128
};

/** Builds canvas-backed text sprites, caching one material per (text, style) so repeated labels share GPU resources. */
export class TextSpriteFactory {
    private readonly cache = new Map<string, THREE.SpriteMaterial>();

    /** A new sprite showing `text`; callers position and scale it. Identical (text, style) share a cached material. */
    create(text: string, style?: TextSpriteStyle): THREE.Sprite {
        return new THREE.Sprite(this.material(text, style));
    }

    private material(text: string, style?: TextSpriteStyle): THREE.SpriteMaterial {
        const s = { ...DEFAULT_STYLE, ...style };
        const key = `${text}|${s.font}|${s.color}|${style?.background ?? ''}|${s.canvasWidth}x${s.canvasHeight}`;
        const cached = this.cache.get(key);
        if (cached) return cached;

        const canvas = document.createElement('canvas');
        canvas.width = s.canvasWidth;
        canvas.height = s.canvasHeight;
        const ctx = canvas.getContext('2d')!;
        if (style?.background) {
            ctx.fillStyle = style.background;
            ctx.fillRect(0, 0, canvas.width, canvas.height);
        }
        ctx.font = s.font;
        ctx.fillStyle = s.color;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(text, canvas.width / 2, canvas.height / 2);

        const material = new THREE.SpriteMaterial({
            map: new THREE.CanvasTexture(canvas),
            depthTest: false,
            depthWrite: false
        });
        this.cache.set(key, material);
        return material;
    }

    /** Frees every cached material and its texture. Sprites created here must already be removed from the scene. */
    dispose(): void {
        for (const material of this.cache.values()) {
            material.map?.dispose();
            material.dispose();
        }
        this.cache.clear();
    }
}
