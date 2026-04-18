/**
 * ThemeProvider — System-aware dark/light mode for If2Ai.
 *
 * Reads the OS `prefers-color-scheme` preference and applies the `dark`
 * class to `<html>` so all Tailwind `dark:` variants and the `.dark {}`
 * CSS token block take effect.
 *
 * Usage:
 *   <ThemeProvider><App /></ThemeProvider>
 *
 * Consumers can call `useTheme()` to read or override the current theme.
 */

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'

type Theme = 'light' | 'dark' | 'system'

interface ThemeContextValue {
  /** Resolved theme: always 'light' or 'dark' (never 'system'). */
  resolvedTheme: 'light' | 'dark'
  /** User preference: 'light', 'dark', or 'system' (follow OS). */
  theme: Theme
  /** Override the theme preference. */
  setTheme: (theme: Theme) => void
}

const ThemeContext = createContext<ThemeContextValue>({
  resolvedTheme: 'light',
  theme: 'system',
  setTheme: () => {},
})

const STORAGE_KEY = 'if2ai-theme'

function getStoredTheme(): Theme {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored === 'light' || stored === 'dark' || stored === 'system') {
      return stored
    }
  } catch {
    // localStorage not available (e.g., SSR or permissions)
  }
  return 'system'
}

function resolveTheme(preference: Theme, systemDark: boolean): 'light' | 'dark' {
  if (preference === 'system') return systemDark ? 'dark' : 'light'
  return preference
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

  const resolvedTheme = resolveTheme(theme, systemDark)

  // Apply / remove the `dark` class on <html>.
  useEffect(() => {
    const root = document.documentElement
    if (resolvedTheme === 'dark') {
      root.classList.add('dark')
    } else {
      root.classList.remove('dark')
    }
  }, [resolvedTheme])

  const setTheme = useCallback((next: Theme) => {
    setThemeState(next)
    try {
      localStorage.setItem(STORAGE_KEY, next)
    } catch {
      // ignore write errors
    }
  }, [])

  return (
    <ThemeContext.Provider value={{ resolvedTheme, theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

/** Hook to read and control the current theme. */
export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext)
}
