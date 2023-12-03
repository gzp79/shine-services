import * as fs from 'fs';

function solutionA(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    let value1 = 0;

    const mp: string[] = [];
    for (let line of text.split('\n')) {
        line = line.trim();
        if (line.length > 0) {
            line = '.' + line + '.';
            //console.log('in:', line);
            mp.push(line);
        }
    }
    for (const l of mp) console.log(l);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solutionA('advent/04/test1.txt');
    });
});
