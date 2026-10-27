import { gsap } from 'gsap'
import { ScrollTrigger } from 'gsap/ScrollTrigger'
import { useEffect, useRef } from 'react'
import {
  ACESFilmicToneMapping,
  BackSide,
  Color,
  DoubleSide,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshPhysicalMaterial,
  PMREMGenerator,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  SphereGeometry,
  Vector3,
  WebGLRenderer,
} from 'three'
import { KEY_WIDTH, buildKeyShards } from '@/components/home/keyGeometry'

gsap.registerPlugin(ScrollTrigger)

const FOV = 28
const MAX_DPR = 1.75
const MAX_KEY_PX = 540
const KEY_CENTER = 0.35

interface KeySceneProps {
  trigger: React.RefObject<HTMLElement | null>
  onReady: () => void
  onFail: () => void
}

const smooth = (a: number, b: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)))
  return t * t * (3 - 2 * t)
}

// Studio lighting built from coloured softboxes, so the glass has something to reflect without an HDR download.
function lightBoxes() {
  const scene = new Scene()
  scene.add(new Mesh(new SphereGeometry(20, 32, 16), new MeshBasicMaterial({ color: 0x0a0a12, side: BackSide })))
  const panels: Array<[number, number, number, number, number, number]> = [
    [0, 9, 3, 14, 4, 0xffffff],
    [-9, 1, 3, 4, 12, 0x4fd1ff],
    [9, -1, 2, 4, 12, 0xff5fd2],
    [0, -8, -4, 14, 3, 0xf4b728],
    [-3, 2, -9, 10, 5, 0xa78bfa],
    [4, 3, 10, 3, 9, 0xffffff],
  ]
  for (const [x, y, z, w, h, hex] of panels) {
    const mat = new MeshBasicMaterial({ color: new Color(hex).multiplyScalar(5), side: DoubleSide })
    const mesh = new Mesh(new PlaneGeometry(w, h), mat)
    mesh.position.set(x, y, z)
    mesh.lookAt(0, 0, 0)
    scene.add(mesh)
  }
  return scene
}

export default function KeyScene({ trigger, onReady, onFail }: KeySceneProps) {
  const hostRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const host = hostRef.current
    const triggerEl = trigger.current
    if (!host || !triggerEl) return

    let renderer: WebGLRenderer
    try {
      renderer = new WebGLRenderer({ alpha: true, antialias: true, powerPreference: 'high-performance' })
    } catch {
      onFail()
      return
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, MAX_DPR))
    renderer.toneMapping = ACESFilmicToneMapping
    renderer.toneMappingExposure = 1.05
    renderer.domElement.className = 'absolute inset-0 size-full'
    host.appendChild(renderer.domElement)

    const envScene = lightBoxes()
    const pmrem = new PMREMGenerator(renderer)
    const envTarget = pmrem.fromScene(envScene, 0.03)
    envScene.traverse((o) => {
      if (o instanceof Mesh) {
        o.geometry.dispose()
        o.material.dispose()
      }
    })
    pmrem.dispose()

    const scene = new Scene()
    scene.environment = envTarget.texture
    const camera = new PerspectiveCamera(FOV, 1, 0.1, 100)

    const material = new MeshPhysicalMaterial({
      color: 0xffffff,
      metalness: 0,
      specularIntensity: 1,
      roughness: 0.05,
      transmission: 1,
      thickness: 2.2,
      dispersion: 4,
      ior: 1.45,
      iridescence: 1,
      iridescenceIOR: 1.7,
      iridescenceThicknessRange: [100, 900],
      clearcoat: 1,
      clearcoatRoughness: 0.08,
      // Dark smoked glass: the body absorbs, so colour lives on the bevels and edges.
      attenuationColor: new Color(0xa8b0d8),
      attenuationDistance: 1.8,
      envMapIntensity: 2.2,
      transparent: true,
    })

    const rand = (() => {
      let s = 11
      return () => ((s = (s * 16807) % 2147483647) - 1) / 2147483646
    })()
    const group = new Group()
    const shards = buildKeyShards().map((shard) => {
      const mesh = new Mesh(shard.geometry, material)
      mesh.position.copy(shard.origin)
      group.add(mesh)
      const dir = new Vector3(shard.origin.x * 0.7, shard.origin.y * 1.4, 0).add(
        new Vector3(rand() - 0.5, rand() - 0.5, (rand() - 0.3) * 4),
      )
      dir.multiplyScalar(1.5 + rand())
      const spin = new Vector3(rand() - 0.5, rand() - 0.5, rand() - 0.5).multiplyScalar(7)
      return { mesh, origin: shard.origin, dir, spin, phase: rand() * Math.PI * 2 }
    })
    scene.add(group)

    const pointer = { x: 0, y: 0, tx: 0, ty: 0 }
    const onPointer = (e: PointerEvent) => {
      pointer.tx = (e.clientX / window.innerWidth - 0.5) * 2
      pointer.ty = (e.clientY / window.innerHeight - 0.5) * 2
    }
    window.addEventListener('pointermove', onPointer, { passive: true })

    let target = 0
    let shown = 0
    let away = 0
    let greyShown = 0
    let time = 0
    const st = ScrollTrigger.create({
      trigger: triggerEl,
      start: 'top top',
      end: 'bottom bottom',
      onUpdate: (self) => {
        target = self.progress
      },
    })

    const resize = () => {
      const w = host.clientWidth
      const h = host.clientHeight
      if (!w || !h) return
      renderer.setSize(w, h, false)
      camera.aspect = w / h
      // Distance that makes the key span min(88% of width, MAX_KEY_PX) on screen
      const keyPx = Math.min(w * 0.88, MAX_KEY_PX)
      const visW = (KEY_WIDTH * w) / keyPx
      const visH = visW / camera.aspect
      camera.position.set(0, 0, visW / camera.aspect / (2 * Math.tan((FOV * Math.PI) / 360)))
      camera.updateProjectionMatrix()
      group.position.y = (0.5 - KEY_CENTER) * visH
    }
    const ro = new ResizeObserver(resize)
    ro.observe(host)
    resize()

    const pose = (dt: number) => {
      time += dt
      shown += (target - shown) * (1 - Math.exp(-dt * 7))
      pointer.x += (pointer.tx - pointer.x) * (1 - Math.exp(-dt * 4))
      pointer.y += (pointer.ty - pointer.y) * (1 - Math.exp(-dt * 4))
      group.rotation.set(
        0.38 + Math.sin(time * 0.27) * 0.06 + pointer.y * 0.12,
        0.35 + Math.sin(time * 0.35) * 0.5 + pointer.x * 0.22,
        -0.08,
      )
      const e = 1 - Math.pow(1 - shown, 3)
      for (const s of shards) {
        // After the break the shards keep drifting slowly instead of settling
        const drift = e * (0.18 * Math.sin(time * 0.3 + s.phase))
        s.mesh.position.copy(s.origin).addScaledVector(s.dir, e * 2.2 + drift)
        const spin = e * (1 + time * 0.012)
        s.mesh.rotation.set(s.spin.x * spin, s.spin.y * spin, s.spin.z * spin)
      }
      // Shards stay as a dim background layer, then fade out well below the hero
      material.opacity = (1 - 0.62 * smooth(0.45, 0.95, shown)) * (1 - away)
      group.visible = material.opacity > 0.01
      const grey = Math.round(smooth(0.6, 1, shown) * 100) / 100
      if (grey !== greyShown) {
        greyShown = grey
        host.style.filter = grey > 0 ? `grayscale(${grey})` : ''
      }
    }

    let last = performance.now()
    const frame = (now: number) => {
      pose(Math.min((now - last) / 1000, 0.1))
      last = now
      renderer.render(scene, camera)
    }
    let visible = true
    const sync = () => {
      const run = visible && !document.hidden && away < 1
      last = performance.now()
      renderer.setAnimationLoop(run ? frame : null)
    }
    const io = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting
      sync()
    })
    io.observe(host)
    document.addEventListener('visibilitychange', sync)
    const fade = ScrollTrigger.create({
      trigger: triggerEl,
      start: 'bottom bottom',
      end: () => `+=${window.innerHeight * 1.6}`,
      invalidateOnRefresh: true,
      onUpdate: (self) => {
        const gone = self.progress >= 1
        const wasGone = away >= 1
        away = self.progress
        if (gone !== wasGone) {
          host.style.visibility = gone ? 'hidden' : 'visible'
          sync()
        }
      },
    })

    const onLost = (e: Event) => {
      e.preventDefault()
      onFail()
    }
    renderer.domElement.addEventListener('webglcontextlost', onLost)

    pose(0)
    renderer.render(scene, camera)
    sync()
    onReady()

    if (import.meta.env.DEV) {
      // Dev only: renders the idle pose to a PNG data URL for public/key-still.png
      Object.assign(window, {
        __keyStill: () => {
          time = 0
          shown = target = 0
          pointer.x = pointer.y = pointer.tx = pointer.ty = 0
          pose(0)
          renderer.render(scene, camera)
          return renderer.domElement.toDataURL('image/png')
        },
      })
    }

    return () => {
      renderer.setAnimationLoop(null)
      io.disconnect()
      ro.disconnect()
      st.kill()
      fade.kill()
      document.removeEventListener('visibilitychange', sync)
      window.removeEventListener('pointermove', onPointer)
      renderer.domElement.removeEventListener('webglcontextlost', onLost)
      for (const s of shards) s.mesh.geometry.dispose()
      material.dispose()
      envTarget.dispose()
      renderer.dispose()
      renderer.forceContextLoss()
      renderer.domElement.remove()
    }
  }, [trigger, onReady, onFail])

  return <div ref={hostRef} aria-hidden="true" className="pointer-events-none fixed inset-0 -z-10" />
}
