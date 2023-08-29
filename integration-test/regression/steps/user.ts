import { KarateState, request, expect } from '../../karate/karate';
import { binding, given, then } from 'cucumber-tsflow';

@binding([KarateState])
export class UserInfoSteps {
    constructor(private karate: KarateState) {}

    private async getUserInfo(sid: string): Promise<any> {
        let response = await request(this.karate.properties.identityUrl)
            .get('/api/auth/user/info')
            .set('Cookie', [`sid=${sid}`])
            .send();
        expect(response).to.have.status(200);
        expect(response).to.be.json;
        return response.body;
    }

    @given('with karate plugin userinfo')
    step_registerUserInfo() {
        this.karate.setProperty(
            'getUserInfo',
            async (sid: string): Promise<any> => await this.getUserInfo(sid)
        );
    }

    @then('match user {jsonExpr} is a guest account')
    async step_expectGuestUser(user: any) {
        expect(user?.userId, 'User id').to.be.uuid('v4');
        expect(user?.name, 'Guest name').to.startWith('Freshman_');
        expect(user?.sessionLength, 'Session').to.be.greaterThanOrEqual(0);
        expect(user?.roles, 'Roles').to.be.empty;
    }

    @then('match user {jsonExpr} equals to {jsonExpr}')
    async step_expectUser(userA: any, userB: any) {
        expect(userA).to.be.deep.equal(userB);
    }
}
