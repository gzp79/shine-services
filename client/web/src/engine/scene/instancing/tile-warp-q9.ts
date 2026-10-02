import { cos, cross, float, max, min, mix, sin, sqrt, step, vec2, vec3 } from 'three/tsl';
import type { Node } from 'three/webgpu';

type Vec2Node = Node<'vec2'>;
type Vec3Node = Node<'vec3'>;
type FloatNode = Node<'float'>;

/** The 9 Q9 nodes in [u][v] grid order (u, v each 0, 0.5, 1): flat index = uIdx * 3 + vIdx. */
export type Q9Points = readonly Vec2Node[];

/**
 * Builds the 9 Q9 nodes from the 4 tile corners (row-major cp0=BL, cp1=BR, cp2=TL, cp3=TR) and the 4
 * per-edge blend factors `a` (Rust convention: `mid = a * start + (1 - a) * end`), one per edge in
 * canonical order [bottom(cp0->cp1), right(cp1->cp3), top(cp3->cp2), left(cp2->cp0)]. The center node
 * is the average of the 4 corners.
 */
export function computeQ9ControlPoints(corners: readonly Vec2Node[], edgeBlends: readonly FloatNode[]): Q9Points {
    const [cp0, cp1, cp2, cp3] = corners;
    const [aBottom, aRight, aTop, aLeft] = edgeBlends;

    const midBottom = mix(cp1, cp0, aBottom);
    const midRight = mix(cp3, cp1, aRight);
    const midTop = mix(cp2, cp3, aTop);
    const midLeft = mix(cp0, cp2, aLeft);
    const center = cp0.add(cp1).add(cp2).add(cp3).mul(0.25);

    return [cp0, midLeft, cp2, midBottom, center, midTop, cp1, midRight, cp3];
}

/** 1D quadratic Lagrange basis at nodes {0, 0.5, 1}. */
function quadraticBasis(t: FloatNode): readonly FloatNode[] {
    const t2 = t.mul(t);
    return [t2.mul(2).sub(t.mul(3)).add(1), t.sub(t2).mul(4), t2.mul(2).sub(t)];
}

/** Derivative of `quadraticBasis`. */
function quadraticBasisDerivative(t: FloatNode): readonly FloatNode[] {
    return [t.mul(4).sub(3), float(4).sub(t.mul(8)), t.mul(4).sub(1)];
}

/** A 2D Q9 distortion: the interpolated point plus its Jacobian columns (∂/∂u, ∂/∂v). */
export type Q9Distortion2D = {
    readonly distortion: Vec2Node;
    readonly jacobian: { readonly dU: Vec2Node; readonly dV: Vec2Node };
};

/** Biquadratic (Q9) Lagrange interpolation of `points` at local (u, v), plus its Jacobian. */
export function lagrangeQ9(points: Q9Points, u: FloatNode, v: FloatNode): Q9Distortion2D {
    const nu = quadraticBasis(u);
    const nv = quadraticBasis(v);
    const dNu = quadraticBasisDerivative(u);
    const dNv = quadraticBasisDerivative(v);

    const weightedSum = (wu: readonly FloatNode[], wv: readonly FloatNode[]): Vec2Node => {
        let sum: Vec2Node = vec2(0, 0);
        for (let i = 0; i < 3; i++) {
            for (let j = 0; j < 3; j++) {
                sum = sum.add(points[i * 3 + j].mul(wu[i].mul(wv[j])));
            }
        }
        return sum;
    };

    return {
        distortion: weightedSum(nu, nv),
        jacobian: { dU: weightedSum(dNu, nv), dV: weightedSum(nu, dNv) }
    };
}

export type WarpQ9Result = {
    readonly position: Vec3Node;
    readonly normal: Vec3Node;
};

/**
 * Scales local z by `height` (xy is unaffected by z, so the mesh is distorted only in the xy plane) and
 * derives the normal from the 2D distortion's Jacobian (dU, dV) plus the constant dZ column, via the
 * same cross-product trick used for the trilinear warp. Shared tail for every 2D distortion below.
 */
function combineWithHeight(
    distortion2D: Q9Distortion2D,
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    const {
        distortion: xy,
        jacobian: { dU, dV }
    } = distortion2D;

    const position = vec3(xy.x, xy.y, height.mul(local.z));
    const dPosDu = vec3(dU.x, dU.y, 0);
    const dPosDv = vec3(dV.x, dV.y, 0);
    const dPosDz = vec3(0, 0, height);

    const n = normalLocal;
    const normal = cross(dPosDv, dPosDz)
        .mul(n.x)
        .add(cross(dPosDz, dPosDu).mul(n.y))
        .add(cross(dPosDu, dPosDv).mul(n.z));

    return { position, normal };
}

/** Standalone Q9 tile warp: interpolates local (x, y) via the 9-node Q9 Lagrange basis. */
export function warpQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    const points = computeQ9ControlPoints(corners, edgeBlends);
    return combineWithHeight(lagrangeQ9(points, local.x, local.y), height, local, normalLocal);
}

/** Cosine bell window on [0, 1]: 0 at t=0 and t=1, peaking at 1 at t=0.5. */
function cosineBell(t: FloatNode): FloatNode {
    return float(0.5).sub(cos(t.mul(Math.PI * 2)).mul(0.5));
}

/** Derivative of `cosineBell`. */
function cosineBellDerivative(t: FloatNode): FloatNode {
    return sin(t.mul(Math.PI * 2)).mul(Math.PI);
}

/**
 * Per-edge offset of the blended mid-edge point from the straight (a=0.5) midpoint, one per edge in
 * canonical order [bottom(cp0-cp1), right(cp1-cp3), top(cp3-cp2), left(cp2-cp0)]. Since `mid = a*start +
 * (1-a)*end`, the offset from the a=0.5 midpoint reduces to `(a-0.5)*(start-end)`.
 */
export function computeCoonsEdgeOffsets(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[]
): readonly Vec2Node[] {
    const [cp0, cp1, cp2, cp3] = corners;
    const [aBottom, aRight, aTop, aLeft] = edgeBlends;

    return [
        cp0.sub(cp1).mul(aBottom.sub(0.5)),
        cp1.sub(cp3).mul(aRight.sub(0.5)),
        cp3.sub(cp2).mul(aTop.sub(0.5)),
        cp2.sub(cp0).mul(aLeft.sub(0.5))
    ];
}

/**
 * Coons-style interpolation of the 4 corners with each edge's offset (see `computeCoonsEdgeOffsets`)
 * blended in through a cosine-bell window along its own edge, damped linearly towards the opposite
 * edge. Exactly reproduces the corners and the 4 mid-edge points, same as `lagrangeQ9`, but stays
 * bounded in between — unlike the Lagrange Q9 basis, whose Jacobian can overshoot when an edge blend is
 * far from 0.5, since a quadratic node's parameter (t=0.5) then no longer matches its physical position.
 */
export function coonsBellQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const [cp0, cp1, cp2, cp3] = corners;
    const [oBottom, oRight, oTop, oLeft] = computeCoonsEdgeOffsets(corners, edgeBlends);

    const c0 = mix(cp0, cp1, u);
    const c1 = mix(cp2, cp3, u);
    const bilinear = mix(c0, c1, v);
    const dBilinearDu = mix(cp1.sub(cp0), cp3.sub(cp2), v);
    const dBilinearDv = c1.sub(c0);

    const bellU = cosineBell(u);
    const bellV = cosineBell(v);
    const dBellU = cosineBellDerivative(u);
    const dBellV = cosineBellDerivative(v);
    const oneMinusU = float(1).sub(u);
    const oneMinusV = float(1).sub(v);

    const edgeOffset = oBottom
        .mul(bellU.mul(oneMinusV))
        .add(oTop.mul(bellU.mul(v)))
        .add(oLeft.mul(bellV.mul(oneMinusU)))
        .add(oRight.mul(bellV.mul(u)));

    const dEdgeOffsetDu = oBottom
        .mul(dBellU.mul(oneMinusV))
        .add(oTop.mul(dBellU.mul(v)))
        .sub(oLeft.mul(bellV))
        .add(oRight.mul(bellV));

    const dEdgeOffsetDv = oBottom
        .mul(bellU)
        .mul(-1)
        .add(oTop.mul(bellU))
        .add(oLeft.mul(dBellV.mul(oneMinusU)))
        .add(oRight.mul(dBellV.mul(u)));

    // At the center (u=v=0.5) every edge's window overlaps at weight 0.5, so `edgeOffset` alone leaves a
    // residual of half the offsets' sum there and the center drifts off the corners' average. `bellU *
    // bellV` vanishes along the whole boundary (u or v = 0 or 1), so subtracting that residual scaled by
    // it cancels exactly at the center without disturbing any corner or mid-edge point.
    const centerResidual = oBottom.add(oTop).add(oLeft).add(oRight).mul(0.5);
    const centerWeight = bellU.mul(bellV);

    const distortion = bilinear.add(edgeOffset).sub(centerResidual.mul(centerWeight));
    const dU = dBilinearDu.add(dEdgeOffsetDu).sub(centerResidual.mul(dBellU.mul(bellV)));
    const dV = dBilinearDv.add(dEdgeOffsetDv).sub(centerResidual.mul(bellU.mul(dBellV)));

    return { distortion, jacobian: { dU, dV } };
}

/** Standalone Coons-style tile warp: interpolates local (x, y) via `coonsBellQ9`. */
export function warpCoonsBell(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(coonsBellQ9(corners, edgeBlends, local.x, local.y), height, local, normalLocal);
}

/**
 * Exact, monotonic reparametrization of t in [0, 1]: warp(0)=0, warp(1)=1, warp(0.5)=m. A linear
 * rational (Mobius) function — the simplification of a degree-2 rational Bezier over control points
 * {0, 0.5, 1} with weights {(1-m)/m, 1, m/(1-m)}. `deriv` is strictly positive for m in (0, 1), and
 * equals exactly 1 at *both* t=0 and t=1 when m=0.5 — i.e. it smoothly reduces to no correction at all
 * as the edge blend approaches 0.5, rather than going to zero speed at the corners regardless of m. An
 * earlier version of this used a degenerate rational Bezier (zero weight on the middle control point)
 * whose derivative was *always* exactly 0 at both endpoints no matter what m was; that unconditional 0
 * exactly canceled the bilinear baseline's corner derivative, leaving a literal zero Jacobian at every
 * tile corner — worse than either `lagrangeQ9` or `coonsBell`. This construction only degrades toward a
 * degenerate Jacobian as m itself approaches the (unavoidably degenerate) 0 or 1 limit.
 */
function rationalWarp(t: FloatNode, m: FloatNode): { warp: FloatNode; deriv: FloatNode } {
    const oneMinusM = float(1).sub(m);
    const denom = oneMinusM.mul(float(1).sub(t)).add(m.mul(t));
    const warp = m.mul(t).div(denom);
    const deriv = m.mul(oneMinusM).div(denom.mul(denom));
    return { warp, deriv };
}

/** `rationalWarp`'s deviation from the identity: 0 at t=0 and t=1, `m - 0.5` at t=0.5. */
function rationalDeviation(t: FloatNode, m: FloatNode): { deviation: FloatNode; derivative: FloatNode } {
    const { warp, deriv } = rationalWarp(t, m);
    return { deviation: warp.sub(t), derivative: deriv.sub(1) };
}

/**
 * Coons-style interpolation like `coonsBellQ9`, but each edge's boundary curve is reparametrized with
 * `rationalWarp` instead of damped with a cosine-bell offset, so it's exact — and, unlike `lagrangeQ9`,
 * always monotonic — along the whole edge, not just at its midpoint. Exactly reproduces the corners and
 * 4 mid-edge points; the center correction is the same cosine-bell bump `coonsBellQ9` uses, since that
 * only ever touches the interior.
 */
export function rationalBezierQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const [cp0, cp1, cp2, cp3] = corners;
    const [aBottom, aRight, aTop, aLeft] = edgeBlends;

    const c0 = mix(cp0, cp1, u);
    const c1 = mix(cp2, cp3, u);
    const bilinear = mix(c0, c1, v);
    const dBilinearDu = mix(cp1.sub(cp0), cp3.sub(cp2), v);
    const dBilinearDv = c1.sub(c0);

    // Each curve is parametrized start-to-end in its own (u or v) direction. Bottom/right run the same
    // way (u/v: 0->1 from cp0/cp1) as the Rust `mid = a*start + (1-a)*end` convention's start->end, so
    // hitting `mid` at t=0.5 needs `m = 1-a`; top/left run the opposite way (u/v: 0->1 from cp2/cp0,
    // i.e. from that edge's `end`), so they need `m = a` directly.
    const bottom = rationalDeviation(u, float(1).sub(aBottom));
    const top = rationalDeviation(u, aTop);
    const left = rationalDeviation(v, aLeft);
    const right = rationalDeviation(v, float(1).sub(aRight));

    const eBottom = cp1.sub(cp0).mul(bottom.deviation);
    const eTop = cp3.sub(cp2).mul(top.deviation);
    const eLeft = cp2.sub(cp0).mul(left.deviation);
    const eRight = cp3.sub(cp1).mul(right.deviation);

    const oneMinusU = float(1).sub(u);
    const oneMinusV = float(1).sub(v);

    const edgeOffset = eBottom.mul(oneMinusV).add(eTop.mul(v)).add(eLeft.mul(oneMinusU)).add(eRight.mul(u));

    const dEdgeOffsetDu = cp1
        .sub(cp0)
        .mul(bottom.derivative.mul(oneMinusV))
        .add(cp3.sub(cp2).mul(top.derivative.mul(v)))
        .sub(eLeft)
        .add(eRight);

    const dEdgeOffsetDv = eTop
        .sub(eBottom)
        .add(cp2.sub(cp0).mul(left.derivative.mul(oneMinusU)))
        .add(cp3.sub(cp1).mul(right.derivative.mul(u)));

    // Same center fix as `coonsBellQ9`: at (0.5, 0.5) each edge curve crosses its own blend point exactly
    // (deviation(0.5) = a - 0.5, matching `computeCoonsEdgeOffsets`), leaving the same residual.
    const [oBottom, oRight, oTop, oLeft] = computeCoonsEdgeOffsets(corners, edgeBlends);
    const centerResidual = oBottom.add(oTop).add(oLeft).add(oRight).mul(0.5);
    const bellU = cosineBell(u);
    const bellV = cosineBell(v);
    const dBellU = cosineBellDerivative(u);
    const dBellV = cosineBellDerivative(v);

    const distortion = bilinear.add(edgeOffset).sub(centerResidual.mul(bellU.mul(bellV)));
    const dU = dBilinearDu.add(dEdgeOffsetDu).sub(centerResidual.mul(dBellU.mul(bellV)));
    const dV = dBilinearDv.add(dEdgeOffsetDv).sub(centerResidual.mul(bellU.mul(dBellV)));

    return { distortion, jacobian: { dU, dV } };
}

/** Standalone rational-Bezier tile warp: interpolates local (x, y) via `rationalBezierQ9`. */
export function warpRationalBezier(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(rationalBezierQ9(corners, edgeBlends, local.x, local.y), height, local, normalLocal);
}

/**
 * Splits the unit square into 4 sub-quads at u=0.5, v=0.5 (bottom-left/right, top-left/right around the
 * center), each an exact bilinear patch through its own 4 of the 9 Q9 points. Every "spoke" from a
 * corner through a mid-edge point to the center is a sub-quad boundary, so it's exactly straight — no
 * additive correction over the whole domain can achieve that while also staying exact at every corner
 * and mid-edge point: the correction would have to be simultaneously zero along that spoke (forced by
 * linearity plus vanishing at both its ends) and nonzero there (to cancel a real leak), which is a
 * contradiction for any correction shape, not just the specific ones tried in `coonsBellQ9` and
 * `rationalBezierQ9`. The trade-off here: only C0 (value-continuous) across the u=0.5/v=0.5 seams, not
 * C1 — the Jacobian can jump there (small for mild edge blends, more visible for extreme ones).
 */
export function piecewiseBilinearQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const points = computeQ9ControlPoints(corners, edgeBlends);
    const p = (i: number, j: number): Vec2Node => points[i * 3 + j];

    const quadU = step(0.5, u);
    const quadV = step(0.5, v);
    const localU = u.mul(2).sub(quadU);
    const localV = v.mul(2).sub(quadV);

    // Column mix picks the sub-quad's two u-extremes (low = mix(P[0],P[1],quadU), high =
    // mix(P[1],P[2],quadU)) for a given v-index; quadU/quadV are exactly 0 or 1 (from `step`), so these
    // mixes are exact selections, equivalent to branching but without an actual branch.
    const colLow = (j: number): Vec2Node => mix(p(0, j), p(1, j), quadU);
    const colHigh = (j: number): Vec2Node => mix(p(1, j), p(2, j), quadU);

    const c00 = mix(colLow(0), colLow(1), quadV);
    const c10 = mix(colHigh(0), colHigh(1), quadV);
    const c01 = mix(colLow(1), colLow(2), quadV);
    const c11 = mix(colHigh(1), colHigh(2), quadV);

    const distortion = mix(mix(c00, c10, localU), mix(c01, c11, localU), localV);
    // The 2x factor is d(localU)/du = d(localV)/dv = 2 within a fixed sub-quad (each spans half the
    // domain); quadU/quadV (hence c00..c11) are treated as locally constant, matching the C0-only claim.
    const dU = mix(c10.sub(c00), c11.sub(c01), localV).mul(2);
    const dV = mix(c01.sub(c00), c11.sub(c10), localU).mul(2);

    return { distortion, jacobian: { dU, dV } };
}

/** Standalone piecewise-bilinear tile warp: interpolates local (x, y) via `piecewiseBilinearQ9`. */
export function warpPiecewiseBilinear(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(piecewiseBilinearQ9(corners, edgeBlends, local.x, local.y), height, local, normalLocal);
}

/** Bilinear interpolation of one sub-quad's 4 corners at local (u, v) — valid for any (u, v), not just
 *  [0, 1]^2, so it can be evaluated outside its own quarter and blended with a neighboring sub-quad. */
function subQuadDistortion(
    c00: Vec2Node,
    c10: Vec2Node,
    c01: Vec2Node,
    c11: Vec2Node,
    localU: FloatNode,
    localV: FloatNode
): Q9Distortion2D {
    const distortion = mix(mix(c00, c10, localU), mix(c01, c11, localU), localV);
    const dU = mix(c10.sub(c00), c11.sub(c01), localV).mul(2);
    const dV = mix(c01.sub(c00), c11.sub(c10), localU).mul(2);
    return { distortion, jacobian: { dU, dV } };
}

/** Blends two distortions field-by-field; `t` exactly 0 or 1 gives an exact pick of `a` or `b`. */
function mixDistortion(a: Q9Distortion2D, b: Q9Distortion2D, t: FloatNode): Q9Distortion2D {
    return {
        distortion: mix(a.distortion, b.distortion, t),
        jacobian: { dU: mix(a.jacobian.dU, b.jacobian.dU, t), dV: mix(a.jacobian.dV, b.jacobian.dV, t) }
    };
}

/** Equal-weight average of 4 distortions. */
function averageDistortion4(
    a: Q9Distortion2D,
    b: Q9Distortion2D,
    c: Q9Distortion2D,
    d: Q9Distortion2D
): Q9Distortion2D {
    return {
        distortion: a.distortion.add(b.distortion).add(c.distortion).add(d.distortion).mul(0.25),
        jacobian: {
            dU: a.jacobian.dU.add(b.jacobian.dU).add(c.jacobian.dU).add(d.jacobian.dU).mul(0.25),
            dV: a.jacobian.dV.add(b.jacobian.dV).add(c.jacobian.dV).add(d.jacobian.dV).mul(0.25)
        }
    };
}

type QuadBlend = {
    readonly home: Q9Distortion2D;
    readonly blended4: Q9Distortion2D;
    readonly blendWeight: FloatNode;
};

/**
 * Shared setup for the `piecewiseBilinear*` family: the hard-selected sub-quad ("home", identical to
 * `piecewiseBilinearQ9`), the equal-weight average of all 4 sub-quads' whole-domain-extended distortions
 * ("blended4"), and a smooth weight that's 1 at the exact center and reaches exactly 0 (with zero
 * derivative, so it introduces no seam of its own) at `radius`. At the exact center, `blended4` reduces
 * to using the full straight spokes (`midRight - midLeft` for dU, `midTop - midBottom` for dV) instead of
 * one sub-quad's one-sided secant — that's the fix; callers decide which of `home`/`blended4`'s fields to
 * actually blend, and how far `radius` should reach.
 */
function blendedSubQuads(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode,
    radius: number
): QuadBlend {
    const points = computeQ9ControlPoints(corners, edgeBlends);
    const p = (i: number, j: number): Vec2Node => points[i * 3 + j];

    const lowU = u.mul(2);
    const highU = u.mul(2).sub(1);
    const lowV = v.mul(2);
    const highV = v.mul(2).sub(1);
    const bl = subQuadDistortion(p(0, 0), p(1, 0), p(0, 1), p(1, 1), lowU, lowV);
    const br = subQuadDistortion(p(1, 0), p(2, 0), p(1, 1), p(2, 1), highU, lowV);
    const tl = subQuadDistortion(p(0, 1), p(1, 1), p(0, 2), p(1, 2), lowU, highV);
    const tr = subQuadDistortion(p(1, 1), p(2, 1), p(1, 2), p(2, 2), highU, highV);

    // quadU/quadV are exactly 0 or 1 (from `step`), so this exactly reproduces the plain (unblended)
    // piecewise-bilinear pick — same as `piecewiseBilinearQ9`.
    const quadU = step(0.5, u);
    const quadV = step(0.5, v);
    const home = mixDistortion(mixDistortion(bl, br, quadU), mixDistortion(tl, tr, quadU), quadV);
    const blended4 = averageDistortion4(bl, br, tl, tr);

    const dx = u.sub(0.5);
    const dy = v.sub(0.5);
    const distanceFromCenter = sqrt(dx.mul(dx).add(dy.mul(dy)));
    const t = min(distanceFromCenter.div(radius), float(1));
    // 1 at t=0 (center), 0 at t=1 (radius and beyond), zero derivative at both ends.
    const blendWeight = float(0.5).add(cos(t.mul(Math.PI)).mul(0.5));

    return { home, blended4, blendWeight };
}

/** Radius (in (u, v) units) around the exact center within which `piecewiseBilinearBlendedQ9` blends
 *  all 4 sub-quads together instead of hard-selecting one — well inside 0.5, the distance from the
 *  center to every other special point (corner/mid-edge), so those stay untouched and exact. */
const CENTER_BLEND_RADIUS = 0.1;

/**
 * Same 4 sub-quads as `piecewiseBilinearQ9` (so still exact at every corner and mid-edge, and spokes
 * are still exactly straight outside the blend zone below), but fixes a specific flaw at the exact
 * center: all 4 sub-quads meet there, yet a hard `step`-based pick always resolves the tie the same way
 * — e.g. always the top-right sub-quad — so the center's Jacobian only ever sees 2 of the 4 surrounding
 * mid-edge points, a visibly biased normal. See `blendedSubQuads` for how the fix works; both position
 * and normal are blended here, confined to a small neighborhood so corner/mid-edge exactness is untouched.
 */
export function piecewiseBilinearBlendedQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const { home, blended4, blendWeight } = blendedSubQuads(corners, edgeBlends, u, v, CENTER_BLEND_RADIUS);
    return mixDistortion(home, blended4, blendWeight);
}

/** Standalone piecewise-bilinear-with-smoothed-center tile warp: interpolates local (x, y) via
 *  `piecewiseBilinearBlendedQ9`. */
export function warpPiecewiseBilinearBlended(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(
        piecewiseBilinearBlendedQ9(corners, edgeBlends, local.x, local.y),
        height,
        local,
        normalLocal
    );
}

/** Radius for `piecewiseBilinearSmoothNormalQ9`'s normal-only blend — much larger than
 *  `CENTER_BLEND_RADIUS` since blending only the normal has no exactness constraint to respect (the
 *  position never moves, so corners/mid-edges can't be disturbed); this just controls how far the
 *  smoothed shading visibly reaches into the tile. Kept under 0.5 so it stops short of the mid-edge
 *  points, leaving their normals — like the corners' — exactly the plain piecewise-bilinear ones. */
const NORMAL_BLEND_RADIUS = 0.4;

/**
 * Same position as `piecewiseBilinearQ9` — completely unblended: hard-selected sub-quad, exact corners
 * and mid-edges, exactly straight spokes everywhere, faceted seams and all. Only the *normal* is blended
 * toward the 4-way average (see `blendedSubQuads`), over `NORMAL_BLEND_RADIUS` — much wider than
 * `piecewiseBilinearBlendedQ9`'s, since there's no geometric exactness to protect. Useful when the
 * faceted-geometry look at the seams is acceptable (or desired) but the shading should smooth out over a
 * visibly larger area around the center than a pinpoint fix would give.
 */
export function piecewiseBilinearSmoothNormalQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const { home, blended4, blendWeight } = blendedSubQuads(corners, edgeBlends, u, v, NORMAL_BLEND_RADIUS);
    return {
        distortion: home.distortion,
        jacobian: {
            dU: mix(home.jacobian.dU, blended4.jacobian.dU, blendWeight),
            dV: mix(home.jacobian.dV, blended4.jacobian.dV, blendWeight)
        }
    };
}

/** Standalone piecewise-bilinear-with-smoothed-normal tile warp: interpolates local (x, y) via
 *  `piecewiseBilinearSmoothNormalQ9`. */
export function warpPiecewiseBilinearSmoothNormal(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(
        piecewiseBilinearSmoothNormalQ9(corners, edgeBlends, local.x, local.y),
        height,
        local,
        normalLocal
    );
}

/**
 * Position from `piecewiseBilinearQ9` (exact at every corner and mid-edge, spokes exactly straight,
 * faceted-but-honest seams) combined with the Jacobian from `coonsBellQ9` (a single smooth C1 formula
 * everywhere — no branch, so no arbitrary tie-break at the center, and bounded/no Runge-style overshoot
 * the way `lagrangeQ9` can have) — the most stable piece of each, at the cost of the two no longer
 * agreeing on what the surface's actual tangent is (the normal isn't the true derivative of *this*
 * distortion's position field, since it's borrowed from a different one).
 */
export function piecewiseBilinearCoonsNormalQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const { distortion } = piecewiseBilinearQ9(corners, edgeBlends, u, v);
    const { jacobian } = coonsBellQ9(corners, edgeBlends, u, v);
    return { distortion, jacobian };
}

/** Standalone piecewise-bilinear-geometry/Coons-normal tile warp: interpolates local (x, y) via
 *  `piecewiseBilinearCoonsNormalQ9`. */
export function warpPiecewiseBilinearCoonsNormal(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(
        piecewiseBilinearCoonsNormalQ9(corners, edgeBlends, local.x, local.y),
        height,
        local,
        normalLocal
    );
}

/**
 * Plain bilinear interpolation of just the 4 corners — no edge-blend or center data at all, so its
 * Jacobian is affine (linear) in the cross direction and structurally cannot oscillate or overshoot:
 * there's no extra curvature information for it to fit in the first place. Same formula
 * `computeTrilinearWarp` uses for its xy component.
 */
function bilinearCornersQ9(corners: readonly Vec2Node[], u: FloatNode, v: FloatNode): Q9Distortion2D {
    const [cp0, cp1, cp2, cp3] = corners;
    const c0 = mix(cp0, cp1, u);
    const c1 = mix(cp2, cp3, u);
    const distortion = mix(c0, c1, v);
    const dU = mix(cp1.sub(cp0), cp3.sub(cp2), v);
    const dV = c1.sub(c0);
    return { distortion, jacobian: { dU, dV } };
}

/**
 * Position from `piecewiseBilinearQ9` (exact at every corner and mid-edge, spokes exactly straight)
 * combined with the Jacobian from the plain 4-corner bilinear map (`bilinearCornersQ9`) — the mildest
 * possible normal field, with zero dependence on the edge-blend data at all, hence nothing for it to
 * oscillate or curve from. Trades away reflecting *where* the boundary crossing actually landed in the
 * shading (an off-center edge blend no longer visibly tilts the normal), in exchange for eliminating
 * every curvature artifact the other normal sources (`lagrangeQ9`, `coonsBellQ9`, `rationalBezierQ9`,
 * and the 4-sub-quad average `piecewiseBilinearBlendedQ9`/`piecewiseBilinearSmoothNormalQ9` use) can
 * introduce — useful when that shading detail is meant to live in the artist-authored model instead,
 * and the mid-edge blend should only ever nudge geometry, never visibly move the light.
 */
export function piecewiseBilinearCornerNormalQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const { distortion } = piecewiseBilinearQ9(corners, edgeBlends, u, v);
    const { jacobian } = bilinearCornersQ9(corners, u, v);
    return { distortion, jacobian };
}

/** Standalone piecewise-bilinear-geometry/corners-only-bilinear-normal tile warp: interpolates local
 *  (x, y) via `piecewiseBilinearCornerNormalQ9`. */
export function warpPiecewiseBilinearCornerNormal(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(
        piecewiseBilinearCornerNormalQ9(corners, edgeBlends, local.x, local.y),
        height,
        local,
        normalLocal
    );
}

/** Half-width of the finite-difference secant `secantJacobian` uses — deliberately large, not
 *  infinitesimal: a wide secant reports the overall trend of `f` across a neighborhood rather than the
 *  instantaneous local slope, which damps both small-scale curvature (finer detail than the secant width
 *  gets averaged out) and how far the normal can diverge from the geometry in a concave dip (a wide
 *  secant cuts across the dip instead of sitting right on its instantaneous tangent) — at the cost of the
 *  normal no longer being any warp's true analytic tangent. */
const SECANT_HALF_WIDTH = 0.25;

/**
 * Estimates the Jacobian of `f` at (u, v) via a clamped central-difference secant of half-width
 * `SECANT_HALF_WIDTH`, instead of an analytic derivative. Sample points are clamped into [0, 1] — a
 * one-sided, correctly-scaled secant near the boundary — rather than extrapolating `f` outside its
 * intended domain; the divisor is the actual (possibly narrowed) sample spacing, so it's never smaller
 * than `SECANT_HALF_WIDTH` and never divides by zero.
 */
function secantJacobian(
    f: (u: FloatNode, v: FloatNode) => Vec2Node,
    u: FloatNode,
    v: FloatNode
): { readonly dU: Vec2Node; readonly dV: Vec2Node } {
    const uPlus = min(u.add(SECANT_HALF_WIDTH), float(1));
    const uMinus = max(u.sub(SECANT_HALF_WIDTH), float(0));
    const vPlus = min(v.add(SECANT_HALF_WIDTH), float(1));
    const vMinus = max(v.sub(SECANT_HALF_WIDTH), float(0));

    const dU = f(uPlus, v).sub(f(uMinus, v)).div(uPlus.sub(uMinus));
    const dV = f(u, vPlus).sub(f(u, vMinus)).div(vPlus.sub(vMinus));
    return { dU, dV };
}

/**
 * Position from `piecewiseBilinearQ9`, completely unblended (exact corners/mid-edges, straight spokes,
 * faceted seams and all). The Jacobian is *not* this (or any) warp's analytic derivative — it's a wide
 * secant of the same plain distortion value (see `secantJacobian`/`SECANT_HALF_WIDTH`), which smooths the
 * seam's hard Jacobian jump into a gradual transition (rather than removing it the way
 * `piecewiseBilinearBlendedQ9` does, confined to a small neighborhood of the center) and, more generally,
 * trades tangent accuracy anywhere on the tile for a calmer, less sharply-reacting normal.
 */
export function piecewiseBilinearSecantNormalQ9(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    u: FloatNode,
    v: FloatNode
): Q9Distortion2D {
    const f = (uu: FloatNode, vv: FloatNode): Vec2Node => piecewiseBilinearQ9(corners, edgeBlends, uu, vv).distortion;
    const distortion = f(u, v);
    const { dU, dV } = secantJacobian(f, u, v);
    return { distortion, jacobian: { dU, dV } };
}

/** Standalone piecewise-bilinear-geometry/secant-normal tile warp: interpolates local (x, y) via
 *  `piecewiseBilinearSecantNormalQ9`. */
export function warpPiecewiseBilinearSecantNormal(
    corners: readonly Vec2Node[],
    edgeBlends: readonly FloatNode[],
    height: FloatNode,
    local: Vec3Node,
    normalLocal: Vec3Node
): WarpQ9Result {
    return combineWithHeight(
        piecewiseBilinearSecantNormalQ9(corners, edgeBlends, local.x, local.y),
        height,
        local,
        normalLocal
    );
}
