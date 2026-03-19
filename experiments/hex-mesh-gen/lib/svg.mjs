// tools/hex-mesh-gen/lib/svg.mjs

export function meshToSVG(points, quads) {
  const polygons = quads.map(quad => {
    const pts = quad.map(i => {
      const x = points[i * 2].toFixed(4);
      const y = points[i * 2 + 1].toFixed(4);
      return `${x},${y}`;
    }).join(' ');
    return `  <polygon points="${pts}" fill="none" stroke="#ccc" stroke-width="0.08"/>`;
  });

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-12 -12 24 24">
${polygons.join('\n')}
</svg>`;
}
