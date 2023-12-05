import * as fs from 'fs';

function solutionA(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    let value1 = 0;

    for (let line of text.split('\n')) {
        line = line.trim();
        if (line.length == 0) {
            continue;
        }

        console.log('in: ', line);
        const [head, tail] = line.split(':');
    }

    console.log('value1:', value1);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solutionA('advent/06/test1.txt');
    });
});
