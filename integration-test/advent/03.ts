import * as fs from 'fs';

function solution(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    text.split('\n');
    let value1 = 0;

    for (let line of text.split('\n')) {
        line = line.trim();
        console.log('in:', line);        
    }
    console.log('value1:', value1);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solution('advent/03/test1.txt');
    });
});
