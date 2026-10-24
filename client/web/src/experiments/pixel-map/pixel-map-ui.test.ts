import { describe, expect, it } from 'vitest';
import { statusText } from './pixel-map-ui';

describe('statusText', () => {
    it('shows the version and the unconfirmed paints when live', () => {
        expect(statusText('live', 'live', 12, 0)).toEqual({ text: 'Live · v12', tone: 'ok' });
        expect(statusText('live', 'live', 12, 2)).toEqual({ text: 'Live · v12 · 2 unconfirmed', tone: 'ok' });
    });

    it('says connecting until the first sync', () => {
        expect(statusText('connecting', 'waiting', 0, 0)).toEqual({ text: 'Connecting…', tone: 'wait' });
    });

    it('says reconnecting while the socket is down, whatever the stream state was', () => {
        expect(statusText('reconnecting', 'live', 5, 1)).toEqual({ text: 'Reconnecting…', tone: 'wait' });
    });

    it('says the stream is unavailable when the server cannot serve it', () => {
        expect(statusText('live', 'unsynced', 5, 0)).toEqual({ text: 'Stream unavailable, retrying', tone: 'error' });
    });

    it('says the connection moved to another tab', () => {
        expect(statusText('replaced', 'live', 5, 0)).toEqual({
            text: 'Opened in another tab, reload to take over',
            tone: 'error'
        });
    });

    it('says disconnected after stop', () => {
        expect(statusText('closed', 'live', 5, 0)).toEqual({ text: 'Disconnected', tone: 'error' });
    });
});
