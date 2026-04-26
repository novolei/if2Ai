#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { spawn } from 'node:child_process'

const repoRoot = resolve(import.meta.dirname, '..')
const referenceImagePath = resolve('/Users/ryanliu/Downloads/platform.png')
const openaiApiKey = process.env.OPENAI_API_KEY
const model = process.env.JIAOCHANG_IMAGE_MODEL || 'gpt-image-2'
const apiBase = process.env.OPENAI_API_BASE || 'https://api.openai.com/v1'

const styleAnchor = [
  'Original high-fidelity pixel art asset for If2Ai Jiaochang.',
  'Strictly match the attached reference image style: bright isometric wuxia courtyard,',
  'white plaster walls, black ceramic tiled roofs, red wooden beams, lanterns, stone paths,',
  'blue waterways, garden plants, warm daylight, clean saturated colors, crisp pixel density.',
  'Do not copy any existing Star Office UI artwork, characters, layout, or assets.',
  'No text, no watermark, no logos, no UI panels. Preserve transparent background when requested.',
].join(' ')

const assets = [
  {
    id: 'background-main',
    out: 'src/assets/jiaochang/backgrounds/shendiao-courtyard.png',
    webp: 'src/assets/jiaochang/backgrounds/shendiao-courtyard.webp',
    size: '1536x1024',
    background: 'opaque',
    prompt:
      `${styleAnchor} Create a wide isometric pixel-art agent operations courtyard map. ` +
      'Central stone training yard shaped like a runtime cockpit, surrounding traditional Chinese academy buildings, small bridge, blue water channels, banners, lanterns, peach blossom, bamboo, and empty readable spaces where UI markers can float. Keep composition calm and legible for overlayed agent positions.',
  },
  {
    id: 'background-mobile',
    out: 'src/assets/jiaochang/backgrounds/shendiao-courtyard-mobile.png',
    webp: 'src/assets/jiaochang/backgrounds/shendiao-courtyard-mobile.webp',
    size: '1024x1536',
    background: 'opaque',
    prompt:
      `${styleAnchor} Create a vertical mobile crop of the same isometric pixel-art Jiaochang courtyard. ` +
      'Prioritize a central training court, water, bridge, black tile roofs, white walls, red timber, lanterns, and spacious top/bottom areas for compact UI overlays.',
  },
  {
    id: 'cover',
    out: 'src/assets/jiaochang/cover.png',
    webp: 'src/assets/jiaochang/cover.webp',
    size: '1024x576',
    background: 'opaque',
    prompt:
      `${styleAnchor} Create a 16:9 cover thumbnail for the Jiaochang decoration drawer. ` +
      'Show the courtyard from an isometric angle with a hero training platform, black roof silhouettes, blue water, red bridge, lanterns, and lively but uncluttered wuxia academy mood.',
  },
  {
    id: 'agent-scholar',
    out: 'src/assets/jiaochang/sprites/agent-scholar.png',
    size: '1024x1024',
    background: 'transparent',
    prompt:
      `${styleAnchor} Create a transparent 4 by 4 sprite sheet of a scholar agent in blue and white wuxia robes with a black scholar hat. ` +
      '16 frames total, evenly spaced grid, idle/walk/think/write poses, isometric pixel character, no background.',
  },
  {
    id: 'agent-swordsman',
    out: 'src/assets/jiaochang/sprites/agent-swordsman.png',
    size: '1024x1024',
    background: 'transparent',
    prompt:
      `${styleAnchor} Create a transparent 4 by 4 sprite sheet of a reviewer swordsman agent in navy robes with subtle sword silhouette. ` +
      '16 frames total, evenly spaced grid, review/guard/inspect/done poses, isometric pixel character, no background.',
  },
  {
    id: 'agent-craftsman',
    out: 'src/assets/jiaochang/sprites/agent-craftsman.png',
    size: '1024x1024',
    background: 'transparent',
    prompt:
      `${styleAnchor} Create a transparent 4 by 4 sprite sheet of a tool-worker craftsman agent in warm ochre and brown robes. ` +
      '16 frames total, evenly spaced grid, build/tool/sync/carry poses, isometric pixel character, no background.',
  },
  ...[
    ['idle', 'quiet moon-white lantern with low glow'],
    ['planning', 'small scroll and ink brush with thinking spark'],
    ['researching', 'open book and blue jade compass'],
    ['executing', 'crossed wooden practice stakes with active spark'],
    ['writing', 'paper scroll with brush stroke'],
    ['syncing', 'two small red lanterns connected by blue current'],
    ['blocked', 'red warning knot and cracked stone token'],
    ['done', 'green jade seal with gold check mark shape'],
  ].map(([status, subject]) => ({
    id: `status-${status}`,
    out: `src/assets/jiaochang/icons/status-${status}.png`,
    size: '512x512',
    background: 'transparent',
    prompt:
      `${styleAnchor} Create a transparent 64x64-friendly pixel icon for agent status "${status}": ${subject}. ` +
      'Centered object, thick readable silhouette, no text, no UI frame, no background.',
  })),
  ...[
    ['lantern', 'red hanging lantern cluster with gold trim'],
    ['banner', 'vertical red-and-gold courtyard banner on wooden pole'],
    ['bridge', 'small arched red wooden bridge segment'],
    ['lotus', 'blue pond lotus leaves and pink flower cluster'],
    ['training-dummy', 'wooden martial training dummy with red cloth tie'],
    ['scroll-table', 'low wooden table with scrolls and tea cup'],
    ['stone-lion', 'small pale stone guardian lion statue'],
  ].map(([name, subject]) => ({
    id: `decor-${name}`,
    out: `src/assets/jiaochang/decor/${name}.png`,
    size: '512x512',
    background: 'transparent',
    prompt:
      `${styleAnchor} Create a transparent isometric pixel-art decoration asset: ${subject}. ` +
      'Match the courtyard reference palette and pixel density, no text, no background.',
  })),
]

function assertReady() {
  if (!openaiApiKey) {
    throw new Error('OPENAI_API_KEY is required to generate FR-008 assets with gpt-image-2.')
  }
  if (!existsSync(referenceImagePath)) {
    throw new Error(`Reference image not found: ${referenceImagePath}`)
  }
}

async function writeAsset(asset, buffer) {
  const outPath = resolve(repoRoot, asset.out)
  await mkdir(dirname(outPath), { recursive: true })
  await writeFile(outPath, buffer)

  if (asset.webp) {
    const webpPath = resolve(repoRoot, asset.webp)
    await mkdir(dirname(webpPath), { recursive: true })
    await convertWithFfmpeg(outPath, webpPath)
  }
}

async function convertWithFfmpeg(inputPath, outputPath) {
  await new Promise((resolvePromise, reject) => {
    const child = spawn('ffmpeg', ['-y', '-i', inputPath, '-compression_level', '6', '-quality', '92', outputPath], {
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stderr = ''
    child.stderr.on('data', (chunk) => {
      stderr += chunk
    })
    child.on('error', reject)
    child.on('close', (code) => {
      if (code === 0) {
        resolvePromise()
      } else {
        reject(new Error(`ffmpeg failed for ${outputPath}: ${stderr}`))
      }
    })
  })
}

async function generateAsset(asset, referenceBlob) {
  const form = new FormData()
  form.set('model', model)
  form.set('prompt', asset.prompt)
  form.set('size', asset.size)
  form.set('background', asset.background)
  form.set('output_format', 'png')
  form.set('n', '1')
  form.set('image', referenceBlob, 'platform.png')

  const response = await fetch(`${apiBase}/images/edits`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${openaiApiKey}`,
    },
    body: form,
  })

  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`${asset.id} generation failed: HTTP ${response.status} ${errorText}`)
  }

  const payload = await response.json()
  const image = payload?.data?.[0]
  if (image?.b64_json) {
    return Buffer.from(image.b64_json, 'base64')
  }
  if (image?.url) {
    const imageResponse = await fetch(image.url)
    if (!imageResponse.ok) {
      throw new Error(`${asset.id} image download failed: HTTP ${imageResponse.status}`)
    }
    return Buffer.from(await imageResponse.arrayBuffer())
  }
  throw new Error(`${asset.id} generation returned no image payload.`)
}

async function main() {
  assertReady()
  const reference = await readFile(referenceImagePath)
  const referenceBlob = new Blob([reference], { type: 'image/png' })

  console.log(`Generating ${assets.length} Jiaochang FR-008 assets with ${model}...`)
  console.log(`Style reference: ${referenceImagePath}`)

  for (const asset of assets) {
    console.log(`-> ${asset.id}: ${asset.out}`)
    const buffer = await generateAsset(asset, referenceBlob)
    await writeAsset(asset, buffer)
  }

  console.log('Jiaochang FR-008 asset generation complete.')
}

main().catch((error) => {
  console.error(error.message)
  process.exit(1)
})
