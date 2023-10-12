import * as request from 'superagent';
import config from '../test.config';
//import requestLogger from 'superagent-logger';

describe('Check auth cookie consistency', () => {
    /*  Scenario Outline: Matrix check for: t:<tid>, s:<sid>, e:<eid>
    Given url (identityUrl)
    * path '/auth/validate'
    * auth cookie matrix '<tid>' '<sid>' '<eid>'
    When method GET
    Then status 200
    * match auth cookie matrix '<expected>'
*/
    const testCases = [
        //-: missing
        //!: not matching (ex different user id). When multiple ! are present in a row it's assumed all of them are different
        //+: ok, valid cookie
        //s: signature mismatch
        // tid | sid | eid | expected | note
        '  s   | -   | -   |          |                                                ',
        '  -   | s   | -   |          |                                                ',
        // '  -   | -   | s   |          |                                                ',
        // '  s   | s   | s   |          |                                                ',
        // '  s   | +   | -   | s        | It is equivalent as not providing a tid at all ',

        // '  -   | -   | -   |          |                                                ',
        // '  -   | -   | +   |          |                                                ',
        // '  -   | -   | !   |          |                                                ',
        // '  -   | +   | -   | s        |                                                ',
        // '  -   | +   | +   | s,e      |                                                ',
        // '  -   | +   | !   | s        |                                                ',
        // '  -   | !   | -   | S        |                                                ',
        // '  -   | !   | +   | S        |                                                ',
        // '  -   | !   | !   | S        |                                                ',

        // '  +   | -   | -   | t        |                                                ',
        // '  +   | -   | +   | t        |                                                ',
        // '  +   | -   | !   | t        |                                                ',
        // '  +   | !   | -   | t        |                                                ',
        // '  +   | !   | +   | t        |                                                ',
        // '  +   | !   | !   | t        |                                                ',
        // '  +   | +   | -   | t,s      |                                                ',
        // '  +   | +   | +   | t,s,e    |                                                ',
        // '  +   | +   | !   | t,s      |                                                ',

        // '  !   | -   | -   | T        |                                                ',
        // '  !   | -   | +   | T        |                                                ',
        // '  !   | -   | !   | T        |                                                ',
        // '  !   | !   | -   | T        |                                                ',
        // '  !   | !   | +   | T        |                                                ',
        // '  !   | !   | !   | T        |                                                ',
        // '  !   | +   | -   | T        |                                                ',
        // '  !   | +   | +   | T        |                                                ',
        // '  !   | +   | !   | T        |                                                '
    ].map((r: string) => r.split('|').map((c: string) => c.trim()));

    it.each(testCases)('Testing cookie matrix [%p,%p,%p]', async (tid, sid, eid, expected) => {
        console.log(tid,sid,eid);
    });
});
