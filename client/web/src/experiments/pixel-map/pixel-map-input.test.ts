import { describe, expect, it } from 'vitest';
import { InputConst } from '../../constants';
import { ClickTracker } from './pixel-map-input';

describe('ClickTracker', () => {
    it('reports a quick press that stayed in place as a click', () => {
        const tracker = new ClickTracker();

        tracker.down(100, 100, 0);

        expect(tracker.up(102, 101, 120)).toBe(true);
    });

    it('does not report a drag', () => {
        const tracker = new ClickTracker();

        tracker.down(100, 100, 0);

        expect(tracker.up(100 + InputConst.MOVE_THRESHOLD_PX + 1, 100, 100)).toBe(false);
    });

    it('does not report a long press', () => {
        const tracker = new ClickTracker();

        tracker.down(100, 100, 0);

        expect(tracker.up(100, 100, InputConst.TAP_THRESHOLD_MS + 1)).toBe(false);
    });

    it('does not report a release without a press', () => {
        expect(new ClickTracker().up(0, 0, 10)).toBe(false);
    });

    it('forgets the press after the release', () => {
        const tracker = new ClickTracker();
        tracker.down(0, 0, 0);
        tracker.up(0, 0, 10);

        expect(tracker.up(0, 0, 20)).toBe(false);
    });

    it('forgets the press when it is cancelled', () => {
        const tracker = new ClickTracker();
        tracker.down(0, 0, 0);

        tracker.cancel();

        expect(tracker.up(0, 0, 10)).toBe(false);
    });
});
