import * as fs from 'fs';

interface Entry {
    sourceStart: number;
    destStart: number;
    length: number;
}

function solutionA(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    let value1 = 0;

    let seeds: number[] = [];
    const seedToSoil: Entry[] = [];
    const soilToFertilizer: Entry[] = [];
    const fertilizerToWater: Entry[] = [];
    const waterToLight: Entry[] = [];
    const lightToTemperature: Entry[] = [];
    const temperatureToHumidity: Entry[] = [];
    const humidityToLocation: Entry[] = [];
    const allMaps = [
        seedToSoil,
        soilToFertilizer,
        fertilizerToWater,
        waterToLight,
        lightToTemperature,
        temperatureToHumidity,
        humidityToLocation
    ];

    let currentEntry: Entry[] | undefined = undefined;
    for (let line of text.split('\n')) {
        line = line.trim();
        if (line.length == 0) {
            continue;
        }

        console.log('in: ', line);
        const [head, tail] = line.split(':');
        if (head == 'seeds') {
            seeds = tail
                .split(' ')
                .map((s) => parseInt(s))
                .filter((s) => !isNaN(s));
        } else if (head.startsWith('seed')) {
            currentEntry = seedToSoil;
        } else if (head.startsWith('soil')) {
            currentEntry = soilToFertilizer;
        } else if (head.startsWith('fertilizer')) {
            currentEntry = fertilizerToWater;
        } else if (head.startsWith('water')) {
            currentEntry = waterToLight;
        } else if (head.startsWith('light')) {
            currentEntry = lightToTemperature;
        } else if (head.startsWith('temperature')) {
            currentEntry = temperatureToHumidity;
        } else if (head.startsWith('humidity')) {
            currentEntry = humidityToLocation;
        } else {
            const [ds, ss, l] = line
                .split(' ')
                .map((s) => parseInt(s))
                .filter((s) => !isNaN(s));
            if (currentEntry == undefined) throw new Error('No current entry');
            currentEntry.push({ destStart: ds, sourceStart: ss, length: l });
        }
    }
    for (const m of allMaps) {
        m.sort((a, b) => a.sourceStart - b.sourceStart);
    }
    console.log('seeds:', seeds);
    console.log('seedToSoil', seedToSoil);
    console.log('soilToFertilizer', soilToFertilizer);
    console.log('fertilizerToWater', fertilizerToWater);
    console.log('waterToLight', waterToLight);
    console.log('lightToTemperature', lightToTemperature);
    console.log('temperatureToHumidity', temperatureToHumidity);
    console.log('humidityToLocation', humidityToLocation);

    let min: number | undefined = undefined;
    for (const seed of seeds) {
        console.log('seed:', seed);
        let value = seed;
        for (const m of allMaps) {
            for (let i = 0; i < m.length; i++) {
                if (value >= m[i].sourceStart && value < m[i].sourceStart + m[i].length) {
                    value = m[i].destStart + (value - m[i].sourceStart);
                    break;
                }
            }
            console.log('   :', value);
        }
        if (min === undefined || value < min) {
            min = value;
        }
    }

    console.log('value1:', min);
}

function solutionB(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');
    let value1 = 0;

    let seeds: number[] = [];
    const seedToSoil: Entry[] = [];
    const soilToFertilizer: Entry[] = [];
    const fertilizerToWater: Entry[] = [];
    const waterToLight: Entry[] = [];
    const lightToTemperature: Entry[] = [];
    const temperatureToHumidity: Entry[] = [];
    const humidityToLocation: Entry[] = [];
    const allMaps = [
        seedToSoil,
        soilToFertilizer,
        fertilizerToWater,
        waterToLight,
        lightToTemperature,
        temperatureToHumidity,
        humidityToLocation
    ];

    let currentEntry: Entry[] | undefined = undefined;
    for (let line of text.split('\n')) {
        line = line.trim();
        if (line.length == 0) {
            continue;
        }

        console.log('in: ', line);
        const [head, tail] = line.split(':');
        if (head == 'seeds') {
            seeds = tail
                .split(' ')
                .map((s) => parseInt(s))
                .filter((s) => !isNaN(s));
        } else if (head.startsWith('seed')) {
            currentEntry = seedToSoil;
        } else if (head.startsWith('soil')) {
            currentEntry = soilToFertilizer;
        } else if (head.startsWith('fertilizer')) {
            currentEntry = fertilizerToWater;
        } else if (head.startsWith('water')) {
            currentEntry = waterToLight;
        } else if (head.startsWith('light')) {
            currentEntry = lightToTemperature;
        } else if (head.startsWith('temperature')) {
            currentEntry = temperatureToHumidity;
        } else if (head.startsWith('humidity')) {
            currentEntry = humidityToLocation;
        } else {
            const [ds, ss, l] = line
                .split(' ')
                .map((s) => parseInt(s))
                .filter((s) => !isNaN(s));
            if (currentEntry == undefined) throw new Error('No current entry');
            currentEntry.push({ destStart: ds, sourceStart: ss, length: l });
        }
    }
    for (const m of allMaps) {
        m.sort((a, b) => a.sourceStart - b.sourceStart);
    }
    console.log('seeds:', seeds);
    console.log('seedToSoil', seedToSoil);
    console.log('soilToFertilizer', soilToFertilizer);
    console.log('fertilizerToWater', fertilizerToWater);
    console.log('waterToLight', waterToLight);
    console.log('lightToTemperature', lightToTemperature);
    console.log('temperatureToHumidity', temperatureToHumidity);
    console.log('humidityToLocation', humidityToLocation);

    let min: number | undefined = undefined;
    for (let j = 0; j < seeds.length; j += 2) {
        const seedStart = seeds[j];
        const seedEnd = seedStart + seeds[j + 1];
        console.log('seed:', seedStart, seedEnd);
        for (let seed = seedStart; seed < seedEnd; seed++) {
            //console.log('seed:', seed);
            let value = seed;
            for (const m of allMaps) {
                for (let i = 0; i < m.length; i++) {
                    if (value >= m[i].sourceStart && value < m[i].sourceStart + m[i].length) {
                        value = m[i].destStart + (value - m[i].sourceStart);
                        break;
                    }
                }
                //console.log('   :', value);
            }
            if (min === undefined || value < min) {
                min = value;
            }
        }
        console.log('min:', min);
    }

    console.log('value1:', min);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solutionA('advent/05/input.txt');
    });
    it('Solves B', async () => {
        solutionB('advent/05/input.txt');
    });
});
