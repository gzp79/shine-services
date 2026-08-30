export interface Application {
    start(): void;
    dispose(): void;
    setInputEnabled?(enabled: boolean): void;
}
