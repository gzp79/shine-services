import * as fs from 'fs';

function solutionA(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    let value1 = 0;

    for (let line of text.split('\n')) {
        line = line.trim();

        console.log('in: ', line);
        const [head, tail] = line.split(':');
        if (tail) {
            const card = parseInt(head.substring('Card '.length).trim());
            console.log('  C:', card);
        }
    }

    console.log('value1:', value1);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solutionA('advent/05/test1.txt');
    });
});
