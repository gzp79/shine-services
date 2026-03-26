// tools/hex-mesh-gen/__tests__/svg.test.mjs
import { describe, it, expect } from 'vitest';
import { meshToSVG } from '../lib/svg.mjs';

describe('meshToSVG', () => {
  it('produces valid SVG with correct viewBox', () => {
    const points = [0, 0, 1, 0, 1, 1, 0, 1];
    const quads = [[0, 1, 2, 3]];
    const svg = meshToSVG(points, quads);
    expect(svg).toContain('viewBox="-12 -12 24 24"');
    expect(svg).toContain('<svg');
    expect(svg).toContain('</svg>');
  });

  it('renders one polygon per quad', () => {
    const points = [0, 0, 1, 0, 1, 1, 0, 1, 2, 0, 2, 1];
    const quads = [[0, 1, 2, 3], [1, 4, 5, 2]];
    const svg = meshToSVG(points, quads);
    const polygonCount = (svg.match(/<polygon/g) || []).length;
    expect(polygonCount).toBe(2);
  });

  it('polygons have fill=none and a stroke', () => {
    const points = [0, 0, 1, 0, 1, 1, 0, 1];
    const quads = [[0, 1, 2, 3]];
    const svg = meshToSVG(points, quads);
    expect(svg).toContain('fill="none"');
    expect(svg).toContain('stroke=');
  });
});
