import { given, binding, after } from 'cucumber-tsflow';
import { CucumberLog, CucumberAttachments } from 'cucumber-tsflow';
import { KarateState, KarateCore, expect } from './karate';
import { MockServer } from './karate';

@binding([CucumberLog, CucumberAttachments, KarateState])
class KarateMock extends KarateCore {
    public constructor(
        logger: CucumberLog,
        logAttachments: CucumberAttachments,
        karate: KarateState
    ) {
        super(logger, logAttachments, karate);
    }

    @given('Start mock server {string} from {string}')
    async step_startMock(mockName: string, mockPath: string) {
        const module = await import(mockPath);
        const server = new module.default();
        expect(server).to.be.instanceof(MockServer);
        await this.karate.startMock(server, undefined);
    }

    @given('Stop mock server {string}')
    async step_stopMock(mockName: string) {
        await this.karate.stopMock(mockName);
    }

    @after()
    async after() {
        await this.karate.stopAllMocks();
    }
}

export = KarateMock;
