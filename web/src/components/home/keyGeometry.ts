import { BufferAttribute, BufferGeometry, ExtrudeGeometry, Path, Shape, Vector3 } from 'three'
import { toCreasedNormals } from 'three/examples/jsm/utils/BufferGeometryUtils.js'

export const KEY_WIDTH = 6.4
const SHARD_COUNT = 30
const MAX_EDGE = 0.2

const BOW_X = -2.2
const BOW_R = 1.1
const SHAFT_HALF = 0.3

// Seeded so the key and its shards are identical on every load.
const rng = (seed: number) => () => {
  seed = (seed + 0x6d2b79f5) | 0
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296
}

function outline() {
  const a0 = Math.asin(SHAFT_HALF / BOW_R)
  const bx = BOW_X + BOW_R * Math.cos(a0)
  const shape = new Shape()
  shape.moveTo(bx, SHAFT_HALF)
  const path: Array<[number, number]> = [
    [2.9, SHAFT_HALF],
    [2.9, -0.9],
    [2.6, -0.9],
    [2.6, -0.5],
    [2.25, -0.5],
    [2.25, -0.8],
    [1.9, -0.8],
    [1.9, -SHAFT_HALF],
    [bx, -SHAFT_HALF],
  ]
  for (const [x, y] of path) shape.lineTo(x, y)
  shape.absarc(BOW_X, 0, BOW_R, -a0, a0 - Math.PI * 2, true)
  const hole = new Path()
  hole.absarc(BOW_X, 0, 0.42, 0, Math.PI * 2, true)
  shape.holes.push(hole)
  return shape
}

interface Tri {
  p: [Vector3, Vector3, Vector3]
  n: [Vector3, Vector3, Vector3]
}

// Bisect the longest edge until every triangle is small enough to be assigned to a shard cleanly.
function tessellate(geometry: BufferGeometry): Array<Tri> {
  const pos = geometry.getAttribute('position')
  const nrm = geometry.getAttribute('normal')
  const stack: Array<Tri> = []
  for (let i = 0; i < pos.count; i += 3) {
    stack.push({
      p: [0, 1, 2].map((k) => new Vector3().fromBufferAttribute(pos, i + k)) as Tri['p'],
      n: [0, 1, 2].map((k) => new Vector3().fromBufferAttribute(nrm, i + k)) as Tri['n'],
    })
  }
  const out: Array<Tri> = []
  for (let tri = stack.pop(); tri; tri = stack.pop()) {
    const { p, n } = tri
    let edge = 0
    let len = 0
    for (let e = 0; e < 3; e++) {
      const l = p[e].distanceTo(p[(e + 1) % 3])
      if (l > len) {
        len = l
        edge = e
      }
    }
    if (len <= MAX_EDGE) {
      out.push(tri)
      continue
    }
    const a = edge
    const b = (edge + 1) % 3
    const c = (edge + 2) % 3
    const mp = p[a].clone().lerp(p[b], 0.5)
    const mn = n[a].clone().add(n[b]).normalize()
    stack.push(
      { p: [p[a], mp, p[c]], n: [n[a], mn, n[c]] },
      { p: [mp, p[b], p[c]], n: [mn, n[b], n[c]] },
    )
  }
  return out
}

export interface Shard {
  geometry: BufferGeometry
  origin: Vector3
}

// Builds the key, then splits it into Voronoi-like shards by assigning small triangles to the nearest seed.
export function buildKeyShards(): Array<Shard> {
  const raw = new ExtrudeGeometry(outline(), {
    depth: 0.6,
    bevelEnabled: true,
    // Deep, round bevels: curved glass catches many env lights, flat glass reads as plastic.
    bevelThickness: 0.28,
    bevelSize: 0.2,
    bevelSegments: 12,
    curveSegments: 48,
  })
  raw.center()
  const creased = toCreasedNormals(raw, Math.PI / 5)
  raw.dispose()
  const tris = tessellate(creased)
  creased.dispose()

  // z is squashed so a shard spans the full thickness of the key
  const centroids = tris.map((t) => t.p[0].clone().add(t.p[1]).add(t.p[2]).divideScalar(3).multiply(new Vector3(1, 1, 0.15)))
  const rand = rng(7)
  const seeds: Array<Vector3> = [centroids[Math.floor(rand() * centroids.length)]]
  const nearest = centroids.map((c) => c.distanceToSquared(seeds[0]))
  while (seeds.length < SHARD_COUNT) {
    let far = 0
    for (let i = 1; i < nearest.length; i++) if (nearest[i] > nearest[far]) far = i
    const seed = centroids[far]
    seeds.push(seed)
    for (let i = 0; i < nearest.length; i++) nearest[i] = Math.min(nearest[i], centroids[i].distanceToSquared(seed))
  }

  const groups: Array<Array<Tri>> = seeds.map(() => [])
  centroids.forEach((c, i) => {
    let best = 0
    let bestD = Infinity
    seeds.forEach((s, k) => {
      const d = c.distanceToSquared(s)
      if (d < bestD) {
        bestD = d
        best = k
      }
    })
    groups[best].push(tris[i])
  })

  return groups
    .filter((g) => g.length > 0)
    .map((group) => {
      const origin = new Vector3()
      for (const t of group) for (const v of t.p) origin.add(v)
      origin.divideScalar(group.length * 3)
      const positions = new Float32Array(group.length * 9)
      const normals = new Float32Array(group.length * 9)
      group.forEach((t, i) => {
        t.p.forEach((v, k) => v.clone().sub(origin).toArray(positions, i * 9 + k * 3))
        t.n.forEach((v, k) => v.toArray(normals, i * 9 + k * 3))
      })
      const geometry = new BufferGeometry()
      geometry.setAttribute('position', new BufferAttribute(positions, 3))
      geometry.setAttribute('normal', new BufferAttribute(normals, 3))
      geometry.computeBoundingSphere()
      return { geometry, origin }
    })
}
