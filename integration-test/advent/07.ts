import * as fs from 'fs';
import { x } from 'joi';

interface Player {
    cards: string;
    bid: number;
    hand: number;
    handName: string;
    hand2: number;
    hand2Name: string;
}

const handNames: string[] = [
    'Invalid',
    'High card',
    'One pair',
    'Two pair',
    'Three of a kind',
    'Full house',
    'Four  of a kind',
    'Five of a kind'
];

const cardRank: Record<string, number> = {
    A: 13,
    K: 12,
    Q: 11,
    J: 10,
    T: 9,
    '9': 8,
    '8': 7,
    '7': 6,
    '6': 5,
    '5': 4,
    '4': 3,
    '3': 2,
    '2': 1
};

const cardRank2: Record<string, number> = {
    A: 13,
    K: 12,
    Q: 11,
    T: 10,
    '9': 9,
    '8': 8,
    '7': 7,
    '6': 6,
    '5': 5,
    '4': 4,
    '3': 3,
    '2': 2,
    J: 1
};

function compareCardRank(card1: string, card2: string) {
    for (let i = 0; i < card1.length; i++) {
        const rank1 = cardRank[card1[i]];
        const rank2 = cardRank[card2[i]];
        if (rank1 > rank2) {
            return 1;
        } else if (rank1 < rank2) {
            return -1;
        }
    }
    throw new Error('Same card: ' + card1 + ' ' + card2);
}

function compareCardRank2(card1: string, card2: string) {
    for (let i = 0; i < card1.length; i++) {
        const rank1 = cardRank2[card1[i]];
        const rank2 = cardRank2[card2[i]];
        if (rank1 > rank2) {
            return 1;
        } else if (rank1 < rank2) {
            return -1;
        }
    }
    throw new Error('Same card: ' + card1 + ' ' + card2);
}

function handOrder(card: string): number {
    const values: Record<string, number> = {};
    for (let i = 0; i < card.length; i++) {
        values[card[i]] = (values[card[i]] ?? 0) + 1;
    }
    const counts: number[] = Object.values(values);
    counts.sort((a, b) => b - a);
    //console.log('  card:', card, counts);
    if (counts[0] == 5) {
        return 7;
    }
    if (counts[0] == 4) {
        return 6;
    }
    if (counts[0] == 3 && counts[1] == 2) {
        return 5;
    }
    if (counts[0] == 3 && counts[1] == 1) {
        return 4;
    }
    if (counts[0] == 2 && counts[1] == 2) {
        return 3;
    }
    if (counts[0] == 2 && counts[1] == 1) {
        return 2;
    }
    return 1;
}

function handOrder2(card: string): number {
    const values: Record<string, number> = {};
    let jokers: number = 0;
    for (let i = 0; i < card.length; i++) {
        if (card[i] === 'J') {
            jokers += 1;
        } else {
            values[card[i]] = (values[card[i]] ?? 0) + 1;
        }
    }
    const counts: number[] = Object.values(values);
    counts.sort((a, b) => b - a);
    //console.log('  card:', card, counts);
    if ((counts[0] ?? 0) + jokers == 5) {
        return 7;
    }
    if (counts[0] + jokers == 4) {
        return 6;
    }
    if (counts[0] + jokers == 3 && counts[1] == 2) {
        return 5;
    }
    if (counts[0] + jokers == 3) {
        return 4;
    }
    if (counts[0] + jokers == 2 && counts[1] == 2) {
        return 3;
    }
    if (counts[0] + jokers == 2 && counts[1] == 1) {
        return 2;
    }
    return 1;
}

function compareCards(p1: Player, p2: Player) {
    if (p1.hand > p2.hand) {
        return 1;
    } else if (p1.hand < p2.hand) {
        return -1;
    } else {
        return compareCardRank(p1.cards, p2.cards);
    }
}

function compareCards2(p1: Player, p2: Player) {
    if (p1.hand2 > p2.hand2) {
        return 1;
    } else if (p1.hand2 < p2.hand2) {
        return -1;
    } else {
        return compareCardRank2(p1.cards, p2.cards);
    }
}

function solutionA(inputFilePath: string) {
    const text: string = fs.readFileSync(inputFilePath, 'utf-8');

    let players: Player[] = [];
    for (let line of text.split('\n')) {
        line = line.trim();
        if (line.length == 0) {
            continue;
        }

        const [cards, bid] = line.split(' ').map((x) => x.trim());
        players.push({
            cards: cards,
            bid: parseInt(bid),
            hand: handOrder(cards),
            handName: handNames[handOrder(cards)],
            hand2: handOrder2(cards),
            hand2Name: handNames[handOrder2(cards)]
        });
    }
    //console.log('players:', players);

    players.sort((a, b) => compareCards(a, b));
    //console.log('players:', players);
    let value1 = 0;
    for (let i = 0; i < players.length; i++) {
        //console.log(players[i]);
        value1 += players[i].bid * (i + 1);
    }
    console.log('value1:', value1);

    players.sort((a, b) => compareCards2(a, b));
    //console.log('players:', players);
    let value2 = 0;
    for (let i = 0; i < players.length; i++) {
        //console.log(players[i]);
        value2 += players[i].bid * (i + 1);
    }
    console.log('value2:', value2);
}

describe('Solution', () => {
    it('Solves A', async () => {
        solutionA('advent/07/input.txt');
    });
});
