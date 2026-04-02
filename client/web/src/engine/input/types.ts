export type Point = { x: number; y: number };
export type Delta = { x: number; y: number };

export type PointerInfo = {
    id: number;
    startPos: Point;
    currentPos: Point;
    downTime: number;
    button: number;
};
