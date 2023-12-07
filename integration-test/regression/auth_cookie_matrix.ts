import { Cookie } from 'tough-cookie';
import api from '$lib/api/api';
import { getCookies } from '$lib/response_utils';

describe('Auth cookie consistency matrix', () => {
    const testCases = [
        //-: missing
        //!: not matching (ex different user id). When multiple ! are present in a row it's assumed all of them are different
        //+: ok, valid cookie
        //s: signature mismatch
        // tid | sid | eid | expected | note
        '  s   | -   | -   |          |                                                ',
        '  -   | s   | -   |          |                                                ',
        '  -   | -   | s   |          |                                                ',
        '  s   | s   | s   |          |                                                ',
        '  s   | +   | -   | s        | It is equivalent as not providing a tid at all ',

        '  -   | -   | -   |          |                                                ',
        '  -   | -   | +   |          |                                                ',
        '  -   | -   | !   |          |                                                ',
        '  -   | +   | -   | s        |                                                ',
        '  -   | +   | +   | s,e      |                                                ',
        '  -   | +   | !   | s        |                                                ',
        '  -   | !   | -   | S        |                                                ',
        '  -   | !   | +   | S        |                                                ',
        '  -   | !   | !   | S        |                                                ',

        '  +   | -   | -   | t        |                                                ',
        '  +   | -   | +   | t        |                                                ',
        '  +   | -   | !   | t        |                                                ',
        '  +   | !   | -   | t        |                                                ',
        '  +   | !   | +   | t        |                                                ',
        '  +   | !   | !   | t        |                                                ',
        '  +   | +   | -   | t,s      |                                                ',
        '  +   | +   | +   | t,s,e    |                                                ',
        '  +   | +   | !   | t,s      |                                                ',

        '  !   | -   | -   | T        |                                                ',
        '  !   | -   | +   | T        |                                                ',
        '  !   | -   | !   | T        |                                                ',
        '  !   | !   | -   | T        |                                                ',
        '  !   | !   | +   | T        |                                                ',
        '  !   | !   | !   | T        |                                                ',
        '  !   | +   | -   | T        |                                                ',
        '  !   | +   | +   | T        |                                                ',
        '  !   | +   | !   | T        |                                                '
    ].map((r: string) => r.split('|').map((c: string) => c.trim()));

    let cookieData!: Record<string, string>;
    beforeAll(async () => {
        // creates a single group of matching cookie triplet (tid,sid,eid)
        const createCookies = async (): Promise<string[]> => {
            let tid, sid, eid: string;

            {
                const response = await api.request.loginWithToken(null, null, true);
                expect(response.statusCode).toEqual(200);
                const cookies = getCookies(response);
                expect(cookies.tid).toBeValidTID();
                tid = cookies.tid.value;
                expect(cookies.sid).toBeValidSID();
                sid = cookies.sid.value;
            }

            //eid
            {
                const response = await api.request.linkWithOAuth2(sid);
                expect(response.statusCode).toEqual(200);
                const cookies = getCookies(response);
                expect(cookies.eid).toBeValidEID();
                eid = cookies.eid.value;
            }

            return [tid, sid, eid];
        };

        const [tid, sid, eid] = await createCookies();
        const tidInvalidSig = 'invalid'.concat(tid.slice(7));
        const sidInvalidSig = 'invalid'.concat(sid.slice(7));
        const eidInvalidSig = 'invalid'.concat(eid.slice(7));
        const [tid2, _sid2, _eid2] = await createCookies();
        const [_tid3, sid2, _eid3] = await createCookies();
        const [_tid4, _sid4, eid2] = await createCookies();

        cookieData = {
            tid,
            sid,
            eid,
            tidInvalidSig,
            sidInvalidSig,
            eidInvalidSig,
            tid2,
            sid2,
            eid2
        };
    });

    it.each(testCases)('Cookie matrix [%p,%p,%p] shall pass', async (tid, sid, eid, expected) => {
        let requestCookies: Record<string, string | null> = {
            tid: null,
            sid: null,
            eid: null
        };

        for (const [c, name] of [
            [tid, 'tid'],
            [sid, 'sid'],
            [eid, 'eid']
        ]) {
            switch (c) {
                case '+':
                    requestCookies[name] = cookieData[name];
                    break;
                case '-':
                    /* noop */ break;
                case '!':
                    requestCookies[name] = cookieData[name + '2'];
                    break;
                case 's':
                    requestCookies[name] = cookieData[name + 'InvalidSig'];
                    break;
                default:
                    throw new Error(`Unhandled cookie mod for ${c}`);
            }
        }
        //console.log(requestCookies);

        const response = await api.request.validate(
            requestCookies.tid,
            requestCookies.sid,
            requestCookies.eid
        );
        expect(response.statusCode).toEqual(200);

        const cookies = getCookies(response);
        const now = new Date().getTime();

        const isValid = (c?: Cookie): boolean => {
            if (!c) return false;
            if (c.expires == 'Infinity') return true;
            return c.expires.getTime() > now;
        };
        const t = isValid(cookies.tid) ? cookies.tid?.value : undefined;
        const s = isValid(cookies.sid) ? cookies.sid?.value : undefined;
        const e = isValid(cookies.eid) ? cookies.eid?.value : undefined;

        const matched = new Set<string>(['t', 's', 'e']);
        // check we have the expected values
        for (const exp of expected.split(',')) {
            switch (exp) {
                case 's':
                    expect(s).toEqual(cookieData.sid);
                    break;
                case 'S':
                    expect(s).toEqual(cookieData.sid2);
                    break;
                case 't':
                    expect(t).toEqual(cookieData.tid);
                    break;
                case 'T':
                    expect(t).toEqual(cookieData.tid2);
                    break;
                case 'e':
                    expect(e).toEqual(cookieData.eid);
                    break;
                case 'E':
                    expect(e).toEqual(cookieData.eid2);
                    break;
            }
            matched.delete(exp.toLowerCase());
        }
        // and no other cookies are valid
        matched.forEach((exp) => {
            switch (exp) {
                case 's':
                    expect(s).not.toBeDefined();
                    break;
                case 't':
                    expect(t).not.toBeDefined();
                    break;
                case 'e':
                    expect(e).not.toBeDefined();
                    break;
            }
        });
    });
});
