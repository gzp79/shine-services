import * as fs from 'fs';

function solutionA(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    let value1 = 0;
    let value2 = 0;

    let copies: Record<number, number> = {};
    for (let line of text.split('\n')) {
        line = line.trim();

        console.log('in: ', line);
        const [head, tail] = line.split(':');
        const card = parseInt(head.substring('Card '.length).trim());
        console.log('  C:', card);
        if (tail) {
            const [winStr, myStr] = tail.split('|');
            const win = winStr
                .split(' ')
                .map((x) => parseInt(x))
                .filter((x) => !isNaN(x));
            const my = myStr
                .split(' ')
                .map((x) => parseInt(x))
                .filter((x) => !isNaN(x));
            //console.log('  W:', win);
            //console.log('  M:', my);
            const cpCount = 1 + (copies[card] ?? 0);
            console.log(' cnt:', cpCount);

            let rowValue = 0;
            let copyId = card + 1;
            for (let i = 0; i < win.length; i++) {
                let w = win[i];
                if (my.some((m) => m === w)) {
                    //console.log('  ', w);
                    rowValue = rowValue === 0 ? 1 : rowValue * 2;
                    copies[copyId] = (copies[copyId] ?? 0) + cpCount;
                    copyId += 1;
                }
            }
            console.log(' S: ', rowValue);
            console.log(' Cp:', copies);
            value1 += rowValue;
            value2 += cpCount;
        }
    }

    console.log('value1:', value1);
    console.log('value2:', value2);
}

describe('Solution', () => {
    it('Solves AB', async () => {
        solutionA('advent/04/input.txt');
    });
});
