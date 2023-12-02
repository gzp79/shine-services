import * as fs from 'fs';

const RED = 12;
const GREEN = 13;
const BLUE = 14;

function solution(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    text.split('\n');
    let value1 = 0;
    let value2 = 0;

    for (let line of text.split('\n')) {
        line = line.trim();
        console.log('in:', line);
        if (!line.startsWith('Game')) continue;

        const [head, tail] = line.split(':');
        const id = parseInt(head.substring('Game '.length));
        console.log(id);

        const groups = tail.split(';').map((grp) => {
            let group: Record<string, number> = {
                red: 0,
                green: 0,
                blue: 0
            };
            grp.split(',').forEach((g) => {
                const [countStr, color] = g.trim().split(' ');
                group[color.trim()] = parseInt(countStr.trim());
            });
            return group;
        });

        let valid = true;
        let minGroup = {
            red: 0,
            green: 0,
            blue: 0
        };
        for (const grp of groups) {
            if (grp.red > RED || grp.green > GREEN || grp.blue > BLUE) {
                valid = false;
            }
            if (grp.red > minGroup.red) minGroup.red = grp.red;

            if (grp.green > minGroup.green) minGroup.green = grp.green;

            if (grp.blue > minGroup.blue) minGroup.blue = grp.blue;
        }
        if (valid) {
            console.log('Valid');
            value1 += id;
        }
        const power = minGroup.red * minGroup.green * minGroup.blue;
        console.log(power);
        value2 += power;
    }
    console.log('value1:', value1);
    console.log('value2:', value2);
}

describe('Solution', () => {
    it('Solves AB', async () => {
        solution('advent/02/input.txt');
    });
});
