export interface GameSystem {
    update(deltaTime: number): void;
    destroy(): void;
}
