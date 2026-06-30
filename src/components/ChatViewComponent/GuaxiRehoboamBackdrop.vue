<template>
  <div class="rehoboam-backdrop" aria-hidden="true">
    <canvas ref="canvasRef" class="rehoboam-backdrop__canvas"></canvas>
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  active: {
    type: Boolean,
    default: false,
  },
})

const logger = createLogger('GuaxiRehoboamBackdrop')
const canvasRef = ref(null)

const TAU = Math.PI * 2
const DPR_CAP = 2

let animationFrameId = 0
let resizeObserver = null
let mediaQuery = null
let mediaQueryChangeHandler = null
let reducedMotion = false
let lastCanvasSizeKey = ''

const EVENT_ANCHORS = [
  { angle: -1.22, strength: 0.74, phase: 0.11 },
  { angle: -0.44, strength: 0.58, phase: 0.32 },
  { angle: 0.28, strength: 0.68, phase: 0.55 },
  { angle: 1.16, strength: 0.82, phase: 0.78 },
  { angle: 2.18, strength: 0.5, phase: 0.24 },
  { angle: 2.94, strength: 0.62, phase: 0.66 },
]

const RING_SPECS = [
  { radius: 0.28, alpha: 0.012, width: 0.32, dash: [2, 5], speed: 0.06, phase: 0.1 },
  { radius: 0.36, alpha: 0.018, width: 0.36, dash: [], speed: -0.08, phase: 0.3 },
  { radius: 0.44, alpha: 0.024, width: 0.44, dash: [3, 8], speed: 0.11, phase: 0.5 },
  { radius: 0.53, alpha: 0.018, width: 0.48, dash: [], speed: -0.14, phase: 0.8 },
  { radius: 0.63, alpha: 0.026, width: 0.62, dash: [5, 10], speed: 0.18, phase: 0.2 },
  { radius: 0.72, alpha: 0.022, width: 0.78, dash: [], speed: -0.22, phase: 0.6 },
]

const OUTER_WAVE_LAYERS = [
  { offset: 0.003, amplitude: 0.045, frequencyA: 18, frequencyB: 50, speed: 0.08, fillAlpha: 0.24, strokeAlpha: 0.58, width: 1.24 },
  { offset: 0.01, amplitude: 0.062, frequencyA: 24, frequencyB: 66, speed: -0.06, fillAlpha: 0.205, strokeAlpha: 0.49, width: 1.06 },
  { offset: 0.017, amplitude: 0.081, frequencyA: 31, frequencyB: 84, speed: 0.07, fillAlpha: 0.168, strokeAlpha: 0.44, width: 0.92 },
  { offset: 0.025, amplitude: 0.106, frequencyA: 38, frequencyB: 108, speed: -0.05, fillAlpha: 0.13, strokeAlpha: 0.39, width: 0.8 },
]

const readTheme = () => {
  const isDark =
    document.documentElement.dataset.guaxiTheme === 'dark' ||
    document.documentElement.classList.contains('dark') ||
    document.body?.classList.contains('dark')

  return {
    isDark,
    background: isDark ? '#17191d' : '#e7e7e4',
    backgroundStart: isDark ? '#202329' : '#f1f1ef',
    backgroundCore: isDark ? 'rgba(241, 241, 239, 0.04)' : 'rgba(255, 255, 255, 0.96)',
    ink: isDark ? 'rgba(241, 241, 239, 0.72)' : 'rgba(16, 16, 16, 0.86)',
    inkStrong: isDark ? 'rgba(241, 241, 239, 0.88)' : 'rgba(4, 4, 4, 0.9)',
    ring: isDark ? 'rgba(241, 241, 239, 0.34)' : 'rgba(16, 16, 16, 0.72)',
    softLine: isDark ? 'rgba(241, 241, 239, 0.11)' : 'rgba(18, 18, 18, 0.14)',
    waveFill: isDark ? 'rgba(241, 241, 239, 0.12)' : 'rgba(16, 16, 16, 0.23)',
    waveStroke: isDark ? 'rgba(241, 241, 239, 0.52)' : 'rgba(4, 4, 4, 0.76)',
  }
}

const resizeCanvas = () => {
  const canvas = canvasRef.value
  if (!canvas) return false

  const rect = canvas.getBoundingClientRect()
  const width = Math.max(1, Math.round(rect.width))
  const height = Math.max(1, Math.round(rect.height))
  const dpr = Math.min(window.devicePixelRatio || 1, DPR_CAP)
  const nextSizeKey = `${width}x${height}@${dpr}`

  if (lastCanvasSizeKey === nextSizeKey) {
    return false
  }

  lastCanvasSizeKey = nextSizeKey
  canvas.width = Math.round(width * dpr)
  canvas.height = Math.round(height * dpr)

  const ctx = canvas.getContext('2d')
  if (ctx) {
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  }

  logger.info('Rehoboam hero replica resized', { width, height, dpr })
  return true
}

const pointAt = (centerX, centerY, angle, radius) => ({
  x: centerX + Math.cos(angle) * radius,
  y: centerY + Math.sin(angle) * radius,
})

const drawRing = (ctx, centerX, centerY, radius, color, width, alpha = 1) => {
  ctx.save()
  ctx.beginPath()
  ctx.arc(centerX, centerY, radius, 0, TAU)
  ctx.strokeStyle = color
  ctx.globalAlpha = alpha
  ctx.lineWidth = width
  ctx.stroke()
  ctx.restore()
}

const traceClosedSmoothPath = (ctx, points) => {
  if (!points.length) return

  const first = points[0]
  const second = points[1] ?? first
  ctx.moveTo((first.x + second.x) / 2, (first.y + second.y) / 2)

  for (let index = 1; index <= points.length; index += 1) {
    const current = points[index % points.length]
    const next = points[(index + 1) % points.length]
    ctx.quadraticCurveTo(current.x, current.y, (current.x + next.x) / 2, (current.y + next.y) / 2)
  }

  ctx.closePath()
}

const angularDistance = (left, right) => Math.abs(Math.atan2(Math.sin(left - right), Math.cos(left - right)))

const createOuterWaveSamples = (centerX, centerY, radius, layer, elapsedSeconds, intensity) => {
  const sampleCount = 420
  const baseRadius = radius * (0.84 + layer.offset)

  return Array.from({ length: sampleCount }, (_, index) => {
    const angle = (index / sampleCount) * TAU
    const carrier = 0.4 + 0.6 * ((Math.sin(angle * 7 + elapsedSeconds * 0.4) + 1) / 2)
    const waveA = Math.pow((Math.sin(angle * layer.frequencyA + elapsedSeconds * layer.speed * TAU) + 1) / 2, 4)
    const waveB = Math.pow((Math.sin(angle * layer.frequencyB - elapsedSeconds * layer.speed * 1.7 * TAU + layer.offset * 180) + 1) / 2, 8)
    const extension = EVENT_ANCHORS.reduce((sum, anchor) => {
      const distance = angularDistance(angle, anchor.angle)
      if (distance > 0.2) return sum
      const envelope = Math.pow(1 - distance / 0.2, 2)
      const pulse = 0.72 + 0.28 * Math.sin(elapsedSeconds * 0.9 + anchor.phase * TAU)
      return sum + envelope * anchor.strength * pulse
    }, 0)
    const ridge = Math.pow(waveA, 4) * 0.46 + Math.pow(waveB, 8) * 0.24
    const needle = Math.pow(Math.max(waveA, waveB), 14) * (0.05 + extension * 1.8)
    const crestHeight = radius * layer.amplitude * carrier * (ridge + needle) * intensity

    return {
      angle,
      base: pointAt(centerX, centerY, angle, baseRadius),
      crest: pointAt(centerX, centerY, angle, baseRadius + crestHeight),
    }
  })
}

const drawBackground = (ctx, width, height, centerX, centerY, radius, theme) => {
  ctx.fillStyle = theme.background
  ctx.fillRect(0, 0, width, height)

  const backdrop = ctx.createLinearGradient(0, 0, 0, height)
  backdrop.addColorStop(0, theme.backgroundStart)
  backdrop.addColorStop(1, theme.background)
  ctx.fillStyle = backdrop
  ctx.fillRect(0, 0, width, height)

  const glow = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, radius * 1.58)
  glow.addColorStop(0, theme.backgroundCore)
  glow.addColorStop(0.56, 'rgba(255, 255, 255, 0)')
  glow.addColorStop(1, 'rgba(255, 255, 255, 0)')
  ctx.fillStyle = glow
  ctx.fillRect(0, 0, width, height)

  drawRing(ctx, centerX, centerY, radius * 0.84, theme.inkStrong, radius * 0.015, theme.isDark ? 0.04 : 0.06)
}

const drawInnerRings = (ctx, centerX, centerY, radius, elapsedSeconds, theme) => {
  RING_SPECS.forEach((ring) => {
    const ringRadius = radius * ring.radius
    const rotation = elapsedSeconds * ring.speed
    const pulse = 0.94 + ((Math.sin(elapsedSeconds * 0.8 + ring.phase * TAU) + 1) / 2) * 0.06

    ctx.save()
    ctx.beginPath()
    ctx.arc(centerX, centerY, ringRadius, rotation, rotation + TAU)
    ctx.strokeStyle = theme.ring
    ctx.globalAlpha = ring.alpha * pulse
    ctx.lineWidth = ring.width
    ctx.setLineDash(ring.dash)
    ctx.lineDashOffset = ringRadius * rotation
    ctx.stroke()
    ctx.restore()
  })

  for (let index = 0; index < 36; index += 1) {
    const angle = (index / 36) * TAU
    const inner = pointAt(centerX, centerY, angle, radius * 0.18)
    const outer = pointAt(centerX, centerY, angle, radius * 0.82)
    ctx.save()
    ctx.beginPath()
    ctx.moveTo(inner.x, inner.y)
    ctx.lineTo(outer.x, outer.y)
    ctx.strokeStyle = theme.softLine
    ctx.globalAlpha = index % 6 === 0 ? 0.22 : 0.12
    ctx.lineWidth = index % 6 === 0 ? 1 : 0.55
    ctx.stroke()
    ctx.restore()
  }
}

const drawOuterWaves = (ctx, centerX, centerY, radius, elapsedSeconds, theme) => {
  const intensity = props.active ? 1.16 : 1

  ctx.save()
  ctx.globalCompositeOperation = theme.isDark ? 'screen' : 'multiply'
  ctx.lineJoin = 'round'
  ctx.lineCap = 'round'

  drawRing(ctx, centerX, centerY, radius * 0.84, theme.waveStroke, Math.max(1.35, radius * 0.0082), theme.isDark ? 0.42 : 0.72)
  drawRing(ctx, centerX, centerY, radius * 0.84, theme.waveStroke, Math.max(2, radius * 0.015), theme.isDark ? 0.15 : 0.28)

  OUTER_WAVE_LAYERS.forEach((layer, layerIndex) => {
    const samples = createOuterWaveSamples(centerX, centerY, radius, layer, elapsedSeconds, intensity)
    const crestPoints = samples.map((sample) => sample.crest)
    const basePoints = [...samples].reverse().map((sample) => sample.base)

    ctx.fillStyle = theme.waveFill
    ctx.globalAlpha = layer.fillAlpha
    ctx.beginPath()
    traceClosedSmoothPath(ctx, crestPoints)
    traceClosedSmoothPath(ctx, basePoints)
    ctx.fill('evenodd')

    ctx.strokeStyle = layerIndex === 0 ? theme.waveStroke : theme.inkStrong
    ctx.globalAlpha = layer.strokeAlpha
    ctx.lineWidth = layer.width
    ctx.beginPath()
    traceClosedSmoothPath(ctx, crestPoints)
    ctx.stroke()
  })

  EVENT_ANCHORS.slice(0, 4).forEach((anchor, index) => {
    const pulse = (Math.sin(elapsedSeconds * 1.3 + anchor.phase * TAU) + 1) / 2
    ctx.save()
    ctx.beginPath()
    ctx.arc(centerX, centerY, radius * (0.89 + index * 0.025), anchor.angle - 0.04 - pulse * 0.03, anchor.angle + 0.04 + pulse * 0.03)
    ctx.strokeStyle = theme.inkStrong
    ctx.globalAlpha = props.active ? 0.42 + pulse * 0.18 : 0.2 + pulse * 0.12
    ctx.lineWidth = 1.2 + pulse * 1.3
    ctx.lineCap = 'round'
    ctx.stroke()
    ctx.restore()
  })

  ctx.restore()
}

const drawSweep = (ctx, centerX, centerY, radius, elapsedSeconds, theme) => {
  const angle = reducedMotion ? -0.35 : elapsedSeconds * -0.26

  ctx.save()
  ctx.beginPath()
  ctx.arc(centerX, centerY, radius * 0.87, angle - 0.052, angle + 0.052)
  ctx.strokeStyle = theme.inkStrong
  ctx.globalAlpha = props.active ? 0.09 : 0.06
  ctx.lineWidth = 0.9
  ctx.stroke()
  ctx.restore()
}

const drawEventDistortions = (ctx, centerX, centerY, radius, elapsedSeconds, theme) => {
  EVENT_ANCHORS.forEach((anchor) => {
    const pulse = (Math.sin(elapsedSeconds * 1.6 + anchor.phase * TAU) + 1) / 2
    const point = pointAt(centerX, centerY, anchor.angle, radius * (0.62 + anchor.strength * 0.2))

    ctx.save()
    ctx.beginPath()
    ctx.arc(point.x, point.y, 2.2 + pulse * 2.6, 0, TAU)
    ctx.fillStyle = theme.inkStrong
    ctx.globalAlpha = props.active ? 0.22 + pulse * 0.16 : 0.12 + pulse * 0.08
    ctx.fill()
    ctx.restore()
  })
}

const drawScene = (time = performance.now()) => {
  const canvas = canvasRef.value
  const ctx = canvas?.getContext('2d')
  if (!canvas || !ctx) return

  resizeCanvas()

  const width = canvas.clientWidth
  const height = canvas.clientHeight
  const centerX = width * 0.5
  const centerY = height * 0.48
  const radius = Math.max(112, Math.min(width, height) * 0.42)
  const elapsedSeconds = time / 1000
  const theme = readTheme()

  ctx.clearRect(0, 0, width, height)

  // 设计思路：这里不再渲染自定义 HUD 文案，而是复刻参考项目首页图本身：
  // 米灰背景、中心环形仪器、外围波动轮廓和低对比度扫描线。
  drawBackground(ctx, width, height, centerX, centerY, radius, theme)
  drawInnerRings(ctx, centerX, centerY, radius, elapsedSeconds, theme)
  drawOuterWaves(ctx, centerX, centerY, radius, elapsedSeconds, theme)
  drawSweep(ctx, centerX, centerY, radius, elapsedSeconds, theme)
  drawEventDistortions(ctx, centerX, centerY, radius, elapsedSeconds, theme)

  if (!reducedMotion) {
    animationFrameId = window.requestAnimationFrame(drawScene)
  }
}

const startRendering = () => {
  window.cancelAnimationFrame(animationFrameId)
  resizeCanvas()
  drawScene()
  logger.info('Rehoboam hero replica started', { reducedMotion, active: props.active })
}

watch(
  () => props.active,
  (active) => {
    logger.info('Rehoboam hero replica active state changed', { active })
    if (reducedMotion) {
      drawScene()
    }
  }
)

onMounted(() => {
  const canvas = canvasRef.value
  mediaQuery = window.matchMedia?.('(prefers-reduced-motion: reduce)') ?? null
  reducedMotion = Boolean(mediaQuery?.matches)

  mediaQueryChangeHandler = (event) => {
    reducedMotion = event.matches
    logger.info('Rehoboam hero replica motion preference changed', { reducedMotion })
    startRendering()
  }
  mediaQuery?.addEventListener?.('change', mediaQueryChangeHandler)

  resizeObserver = new ResizeObserver(() => {
    const changed = resizeCanvas()
    if (changed && reducedMotion) {
      drawScene()
    }
  })

  if (canvas) {
    resizeObserver.observe(canvas)
  }

  startRendering()
})

onBeforeUnmount(() => {
  window.cancelAnimationFrame(animationFrameId)
  if (mediaQuery && mediaQueryChangeHandler) {
    mediaQuery.removeEventListener?.('change', mediaQueryChangeHandler)
  }
  resizeObserver?.disconnect()
  resizeObserver = null
  mediaQuery = null
  mediaQueryChangeHandler = null
  logger.info('Rehoboam hero replica destroyed')
})
</script>

<style scoped>
.rehoboam-backdrop {
  position: absolute;
  inset: 0;
  z-index: 0;
  overflow: hidden;
  pointer-events: none;
  background: linear-gradient(180deg, #f1f1ef 0%, #e7e7e4 100%);
}

.rehoboam-backdrop__canvas {
  display: block;
  width: 100%;
  height: 100%;
}

:global(:root[data-guaxi-theme='dark']) .rehoboam-backdrop,
:global(.dark) .rehoboam-backdrop,
:global(.dark-mode) .rehoboam-backdrop {
  background: linear-gradient(180deg, #202329 0%, #17191d 100%);
}
</style>
