export type ConnectionState = 'connecting' | 'syncing' | 'live' | 'reconnecting' | 'replaced' | 'closed';

export interface StreamListener {
    frame(frame: unknown): void;
    connected(): void;
    disconnected(): void;
}

export interface StreamSubscription {
    subscribe(): Promise<void>;
    sendCommand(commandId: string, command: unknown): boolean;
    unsubscribe(): void;
}

// Scene-facing transport contract. Hosts provide the implementation and own its lifetime.
export interface StreamHub {
    readonly isConnected: boolean;
    createSubscription(streamId: string, createListener: (userId: string) => StreamListener): StreamSubscription;
}
