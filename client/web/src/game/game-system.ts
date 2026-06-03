export interface GameSystem {
    name: string;
    update(deltaTime: number): void;
    dispose(): void;
}
