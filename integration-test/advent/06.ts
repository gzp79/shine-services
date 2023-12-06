import * as fs from 'fs';

function value(t: number, d: number) {
    if (t * t - 4 * d > 0) {
        const maxT = 0.5 * (t + Math.sqrt(t * t - 4 * d));
        const minT = 0.5 * (t - Math.sqrt(t * t - 4 * d));
        console.log('range:', minT, maxT);
        let a = Math.ceil(minT);
        let b = Math.floor(maxT);
        if (a == minT) {
            a += 1;
        }
        if (b == maxT) {
            b -= 1;
        }
        if (b >= a) {
            const value = b - a + 1;
            console.log('range:', a, b, value);
            return value;
        }
    }
    return 0;
}

function solutionA(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    let value1 = 1;

    let times: number[] = undefined!;
    let distances: number[] = undefined!;
    let time2 = 0;
    let distance2 = 0;
    for (let line of text.split('\n')) {
        line = line.trim();
        if (line.length == 0) {
            continue;
        }

        if (line.startsWith('Time:')) {
            times = line
                .substring(5)
                .split(' ')
                .map((s) => parseInt(s.trim()))
                .filter((n) => !isNaN(n));
            time2 = parseInt(
                line
                    .substring(5)
                    .split(' ')
                    .map((s) => s.trim())
                    .join('')
            );
        } else if (line.startsWith('Distance:')) {
            distances = line
                .substring(9)
                .split(' ')
                .map((s) => parseInt(s.trim()))
                .filter((n) => !isNaN(n));
            distance2 = parseInt(
                line
                    .substring(9)
                    .split(' ')
                    .map((s) => s.trim())
                    .join('')
            );
        }
    }

    console.log('times:', times);
    console.log('distances:', distances);
    expect(times.length).toBe(distances.length);

    for (let i = 0; i < times.length; i++) {
        const t = times[i];
        const d = distances[i];
        const val = value(t, d);
        if (val > 0) {
            value1 *= val;
        }
    }
    console.log('value1:', value1);
    console.log('value2:', value(time2, distance2));
}

describe('Solution', () => {
    it('Solves A', async () => {
        solutionA('advent/06/input.txt');
    });
});
