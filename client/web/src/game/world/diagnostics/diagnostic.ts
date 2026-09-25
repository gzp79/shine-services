/** A world diagnostic overlay: constructed with its dependencies, wired to events, and disposed with the world. */
export interface Diagnostic {
    dispose(): void;
}
