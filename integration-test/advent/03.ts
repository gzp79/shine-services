import * as fs from 'fs';

function isDigit(str: string): boolean {
    return /^\d+$/.test(str);
}

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

    for (let y = 0; y < mp.length; y++) {
        for (let x = 1; x < mp[y].length; x++) {
            if (isDigit(mp[y][x]) && !isDigit(mp[y][x - 1])) {
                //start of a number
                let l = 1;
                while (isDigit(mp[y][x + l])) l += 1;
                const n = mp[y].substring(x, x + l);
                console.log('n:', n);

                const top = y > 0 ? mp[y - 1].substring(x - 1, x + l + 1) : '.'.repeat(l + 2);
                const bottom = y + 1 < mp.length ? mp[y + 1].substring(x - 1, x + l + 1) : '.'.repeat(l + 2);
                const left = mp[y][x - 1];
                const right = mp[y][x + l];

                console.log('   ', top);
                console.log('   ', left + n + right);
                console.log('   ', bottom);
                expect(top.length).toEqual(n.length + 2);
                expect(bottom.length).toEqual(n.length + 2);

                let count = false;
                for (const s of [top, left, right, bottom]) {
                    if (!s.split('').every((c) => c === '.')) count = true;
                }
                const v = parseInt(n);
                console.log('   ', n, count);
                if (count) {
                    value1 += v;
                }
            }
        }
    }
    console.log('value1:', value1);
}

function solutionB(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');

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

    let numbers: Record<string, number[]> = {};
    let gc = 0;
    for (let y = 0; y < mp.length; y++) {
        for (let x = 1; x < mp[y].length; x++) {
            if (isDigit(mp[y][x]) && !isDigit(mp[y][x - 1])) {
                //start of a number
                let nl = 1;
                while (isDigit(mp[y][x + nl])) nl += 1;
                const n = mp[y].substring(x, x + nl);
                //console.log('n:', n);

                const top = y > 0 ? mp[y - 1].substring(x - 1, x + nl + 1) : '.'.repeat(nl + 2);
                const t = { t: top, y: y - 1, x };
                const bottom =
                    y + 1 < mp.length ? mp[y + 1].substring(x - 1, x + nl + 1) : '.'.repeat(nl + 2);
                const b = { t: bottom, y: y + 1, x };
                const left = mp[y][x - 1];
                const l = { t: left, y, x: x };
                const right = mp[y][x + nl];
                const r = { t: right, y, x: x + nl + 1 };

                //console.log('   ', top);
                //console.log('   ', left + n + right);
                //console.log('   ', bottom);
                expect(top.length).toEqual(n.length + 2);
                expect(bottom.length).toEqual(n.length + 2);

                for (const s of [t, l, r, b]) {
                    const sp = s.t.indexOf('*');
                    if (sp !== -1) {
                        const sx = s.x + sp - 1;
                        const sy = s.y;
                        // connect each number to the position of the attached star
                        if (numbers[`${sx},${sy}`] === undefined) numbers[`${sx},${sy}`] = [];
                        numbers[`${sx},${sy}`].push(parseInt(n));
                    }
                }
            }
        }
    }

    let value = 0;
    const gears: Record<string, number[]> = {};
    for (const g in numbers) {
        let x = parseInt(g.split(",")[0])
        let y = parseInt(g.split(",")[1])
        if(mp[y][x] !== '*') {
            throw Error(JSON.stringify(g))
        }
        if (numbers[g].length === 2)  {
            value += numbers[g][0] * numbers[g][1]
            gears[g] = numbers[g];
        }
    }
    console.log('value1:', gears);
    console.log('value:', value);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solutionA('advent/03/input.txt');
    });
    it('Solves B', async () => {
        solutionB('advent/03/input.txt');
    });
});
