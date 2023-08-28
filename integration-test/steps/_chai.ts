import chai from 'chai';
import chaiMatchPattern from 'chai-match-pattern';
import deepEqualInAnyOrder from 'deep-equal-in-any-order';
import chaiHttp from 'chai-http';
import chaiString from 'chai-string';
import chaiUUID from 'chai-uuid';
//import chaiSpies from 'chai-spies';

chai.use(chaiMatchPattern);
chai.use(deepEqualInAnyOrder);
chai.use(chaiHttp);
chai.use(chaiString);
chai.use(chaiUUID);
//chai.use(chaiSpies);
const _ = chaiMatchPattern.getLodashModule();

declare global {
    namespace Chai {
        interface Assertion {
            defined(message?: string): Assertion;
            subject<T>(): T;
            beforeTime(expectedTime: Date, message?: string): Assertion;
            afterTime(expectedTime: Date, message?: string): Assertion;
        }
    }
}

// Custom assertion to test if something is defined
chai.Assertion.addProperty('subject', function (): any {
    chai.expect(this._obj).to.be.defined();
    return this._obj;
});

// Custom assertion to test if something is defined
chai.Assertion.addMethod('defined', function (message?: string) {
    const subject = this._obj;
    new chai.Assertion(subject, message).to.not.be.undefined;
});

chai.Assertion.addMethod(
    'beforeTime',
    function (expectedTime: Date, message?: string) {
        let actualTime = this._obj;

        if (typeof actualTime == 'number') {
            actualTime = new Date(actualTime);
        } else if (typeof actualTime == 'string') {
            actualTime = new Date(actualTime);
        }

        chai.expect(actualTime).to.be.a('Date');
        chai.expect(actualTime, message).to.be.lessThan(expectedTime);
    }
);

chai.Assertion.addMethod(
    'afterTime',
    function (expectedTime: Date, message?: string) {
        let actualTime = this._obj;
        if (!(actualTime instanceof Date)) {
            actualTime = new Date(actualTime);
            chai.assert(
                !isNaN(actualTime.getTime()),
                'input could not be converted to a valid Date'
            );
        }

        chai.expect(actualTime, message).to.be.greaterThan(expectedTime);
    }
);

export default chai;
