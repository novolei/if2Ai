// MIG-012 — minimal Node module-resolution hook that maps the
// Vite-style `@/...` path alias onto `<repo>/src/...` so the
// frontend API facade tests can run under `node --test` without
// depending on Vite / vitest.

import { existsSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import { resolve as resolvePath } from 'node:path'

const repoRoot = resolvePath(import.meta.dirname, '..')

const EXT_CANDIDATES = ['.ts', '.tsx', '.mts', '/index.ts', '/index.tsx']

export function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith('@/')) {
    const base = resolvePath(repoRoot, 'src', specifier.slice(2))
    for (const ext of EXT_CANDIDATES) {
      const candidate = base + ext
      if (existsSync(candidate)) {
        return nextResolve(pathToFileURL(candidate).href, context)
      }
    }
    return nextResolve(pathToFileURL(base).href, context)
  }
  return nextResolve(specifier, context)
}
