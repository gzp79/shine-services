import type { GameSystem } from '../engine/game-system';
import type { InputState } from '../engine/input/input-state';

export class ClearInputStateSystem implements GameSystem {
    readonly name = 'Clear Input State';

    constructor(private readonly input: InputState) {}

    update(_deltaTime: number): void {
        this.input.clear();
    }

    dispose(): void {}
}
