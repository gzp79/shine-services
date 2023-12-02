import * as fs from 'fs';

const tokens1 = [
    { p: '0', v: 0 },
    { p: '1', v: 1 },
    { p: '2', v: 2 },
    { p: '3', v: 3 },
    { p: '4', v: 4 },
    { p: '5', v: 5 },
    { p: '6', v: 6 },
    { p: '7', v: 7 },
    { p: '8', v: 8 },
    { p: '9', v: 9 },
];

const tokens2 = [
    ...tokens1,
    { p: 'one', v: 1 },
    { p: 'two', v: 2 },
    { p: 'three', v: 3 },
    { p: 'four', v: 4 },
    { p: 'five', v: 5 },
    { p: 'six', v: 6 },
    { p: 'seven', v: 7 },
    { p: 'eight', v: 8 },
    { p: 'nine', v: 9 }
];

function solution(inputFilePath: string, tokens: any) {
    let value = 0;

    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    text.split('\n');
    for (let line of text.split('\n')) {
        line = line.trim();
        console.log('in:', line);

        let first = 0;
        {
            let firstPos = 99;
            for (const t of tokens) {
                const pos = line.indexOf(t.p);
                if (pos === -1) continue;
                if (firstPos > pos) {
                    firstPos = pos;
                    first = t.v;
                }
            }
        }

        let last = 0;
        {
            const lineRev = line.split('').reverse().join('');
            let lastPos = 99;
            for (const t of tokens) {
                const p = t.p.split('').reverse().join('');
                const pos = lineRev.indexOf(p);
                if (pos === -1) continue;
                if (lastPos > pos) {
                    lastPos = pos;
                    last = t.v;
                }
            }
        }

        const v = 10 * first + last;
        console.log('   ', v);
        value += v;
    }
    console.log('value:', value);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solution('advent/01/input.txt', tokens1)
    });

    it('Solves B', async () => {
        solution('advent/01/input.txt', tokens2)
    });
});
