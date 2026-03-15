import { expect, test } from '$fixtures/setup';
import { TestUser } from '$lib/api/test_user';
import OAuth2MockServer from '$lib/mocks/oauth2';

test.describe('Purge guests', () => {
    test.describe.configure({ mode: 'serial' });

    let admin!: TestUser;

    test.beforeAll(async ({ api }) => {
        // Create a SuperAdmin user to authorize purge calls
        const mock = new OAuth2MockServer();
        await mock.start();
        admin = await api.testUsers.createLinked(mock, { roles: ['SuperAdmin'] });
        await mock.stop();
    });

    test('Unauthorized request shall be rejected', async ({ api }) => {
        const guest = await api.testUsers.createGuest();
        const response = await api.user.purgeGuestsRequest(guest.sid, 'PT0S', 100);
        expect(response).toHaveStatus(403);
    });

    test('Invalid olderThan format shall return 400', async ({ api }) => {
        const response = await api.user.purgeGuestsRequest(admin.sid, 'not-a-duration', 100);
        expect(response).toHaveStatus(400);
    });

    test('Limit out of range shall return 400', async ({ api }) => {
        const response = await api.user.purgeGuestsRequest(admin.sid, 'PT0S', 0);
        expect(response).toHaveStatus(400);
        const response2 = await api.user.purgeGuestsRequest(admin.sid, 'PT0S', 1001);
        expect(response2).toHaveStatus(400);
    });

    test('Purge deletes matching guests and clears their sessions', async ({ api }) => {
        // Create 3 guests tracked by userId
        const guests = await Promise.all([
            api.testUsers.createGuest(),
            api.testUsers.createGuest(),
            api.testUsers.createGuest()
        ]);
        const guestIds = new Set(guests.map((g) => g.userId));

        // Purge with zero duration (all guests) with generous limit
        const result = await api.user.purgeGuests(admin.sid, 'PT0S', 200);
        expect(result.deleted).toBeGreaterThanOrEqual(3);

        // Verify each guest's session is invalidated
        for (const guest of guests) {
            const response = await api.user.getUserInfoRequest(guest.sid, 'fast');
            expect(response).toHaveStatus(401);
        }

        // Session invalidation above is sufficient evidence of deletion
        void guestIds; // tracked for context
    });

    test('Pagination: hasMore=true when limit reached, false when drained', async ({ api }) => {
        // Pre-purge to drain any pre-existing guests
        let drain = await api.user.purgeGuests(admin.sid, 'PT0S', 500);
        while (drain.hasMore) {
            drain = await api.user.purgeGuests(admin.sid, 'PT0S', 500);
        }

        // Create exactly 5 guests
        await Promise.all(Array.from({ length: 5 }, () => api.testUsers.createGuest()));

        // Purge with limit=3 — should report hasMore=true
        const first = await api.user.purgeGuests(admin.sid, 'PT0S', 3);
        expect(first.deleted).toEqual(3);
        expect(first.hasMore).toBe(true);

        // Purge remaining — should drain and hasMore=false
        const second = await api.user.purgeGuests(admin.sid, 'PT0S', 3);
        expect(second.deleted).toEqual(2);
        expect(second.hasMore).toBe(false);
    });

    test('Users with confirmed email are not purged', async ({ api }) => {
        // A linked user has confirmed email via OAuth2 — must not be deleted
        const mock = new OAuth2MockServer();
        await mock.start();
        const linked = await api.testUsers.createLinked(mock);
        await mock.stop();

        // Pre-purge to drain real guests
        let drain = await api.user.purgeGuests(admin.sid, 'PT0S', 500);
        while (drain.hasMore) {
            drain = await api.user.purgeGuests(admin.sid, 'PT0S', 500);
        }

        // Linked user's session must still be valid
        const info = await api.user.getUserInfo(linked.sid, 'fast');
        expect(info.userId).toEqual(linked.userId);
    });
});
