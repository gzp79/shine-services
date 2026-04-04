export interface GameSystem {
    update(deltaTime: number): void;
    dispose(): void;
}
