/**
 * ThemeProvider — named, system-aware themes for If2Ai.
 *
 * Reads the OS `prefers-color-scheme` preference and applies `data-theme`,
 * `data-color-scheme`, and the compatibility `dark` class to `<html>`.
 *
 * Usage:
 *   <ThemeProvider><App /></ThemeProvider>
 *
 * Consumers can call `useTheme()` to read or override the current theme.
 */

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'

export type Theme = 'system' | 'current' | 'warm-paper' | 'qingye' | 'black'
type ResolvedTheme = Exclude<Theme, 'system'>
type ResolvedScheme = 'light' | 'dark'

interface ThemeContextValue {
  /** Resolved theme: always a named theme, never 'system'. */
  resolvedTheme: ResolvedTheme
  /** Resolved color scheme for compatibility with existing dark variants. */
  resolvedScheme: ResolvedScheme
  /** User preference: named theme or 'system' (follow OS). */
  theme: Theme
  /** Override the theme preference. */
  setTheme: (theme: Theme) => void
}

const ThemeContext = createContext<ThemeContextValue>({
  resolvedTheme: 'current',
  resolvedScheme: 'light',
  theme: 'system',
  setTheme: () => {},
})

const STORAGE_KEY = 'if2ai-theme'

function getStoredTheme(): Theme {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (
      stored === 'system' ||
      stored === 'current' ||
      stored === 'warm-paper' ||
      stored === 'qingye' ||
      stored === 'black'
    ) {
      return stored
    }
    if (stored === 'light') return 'current'
    if (stored === 'dark') return 'black'
  } catch {
    // localStorage not available (e.g., SSR or permissions)
  }
  return 'system'
}

function resolveTheme(preference: Theme, systemDark: boolean): ResolvedTheme {
  if (preference === 'system') return systemDark ? 'black' : 'current'
  return preference
}

function themeScheme(theme: ResolvedTheme): ResolvedScheme {
  return theme === 'qingye' || theme === 'black' ? 'dark' : 'light'
}

/** ThemeProvider — apply at the root of the React tree. */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(getStoredTheme)
  const [systemDark, setSystemDark] = useState<boolean>(
    () =>
      typeof window !== 'undefined'
        ? window.matchMedia('(prefers-color-scheme: dark)').matches
        : false,
  )
  // Listen to OS preference changes.
  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    const handler = (e: MediaQueryListEvent) => setSystemDark(e.matches)
    mq.addEventListener('change', handler)
    return () => mq.removeEventListener('change', handler)
  }, [])

  // Settings can run in a separate Tauri webview. Mirror theme updates across
  // windows so the selector feels live instead of applying only after restart.
  useEffect(() => {
    const handler = (event: StorageEvent) => {
      if (event.key === STORAGE_KEY) {
        setThemeState(getStoredTheme())
      }
    }
    window.addEventListener('storage', handler)
    return () => window.removeEventListener('storage', handler)
  }, [])

  const resolvedTheme = resolveTheme(theme, systemDark)
  const resolvedScheme = themeScheme(resolvedTheme)

  // Apply named theme attributes and keep the `dark` class for Tailwind
  // compatibility while older component styles are migrated to variables.
  useEffect(() => {
    const root = document.documentElement
    root.dataset.theme = resolvedTheme
    root.dataset.colorScheme = resolvedScheme
    root.style.colorScheme = resolvedScheme
    if (resolvedScheme === 'dark') {
      root.classList.add('dark')
    } else {
      root.classList.remove('dark')
    }
  }, [resolvedTheme, resolvedScheme])

  const setTheme = useCallback((next: Theme) => {
    setThemeState(next)
    try {
      localStorage.setItem(STORAGE_KEY, next)
    } catch {
      // ignore write errors
    }
  }, [])

  return (
    <ThemeContext.Provider value={{ resolvedTheme, resolvedScheme, theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

/** Hook to read and control the current theme. */
export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext)
}
