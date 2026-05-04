/**
 * Shared dropdown menu item button used by chat-ui pickers.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behaviour change. The styling tokens and active-state colours are
 * preserved verbatim.
 */

import * as React from 'react'
import { cn } from '@/lib/utils'
import { DropdownMenuItem } from '@/components/ui/dropdown-menu'

/**
 * Render a uniformly-styled item inside a chat-ui DropdownMenu. The
 * `active` flag swaps in the accent palette; `onClick` is wired to the
 * underlying `onSelect` so keyboard activation continues to work.
 */
export function MenuItemButton({
  children,
  active = false,
  onClick,
}: {
  children: React.ReactNode
  active?: boolean
  onClick?: () => void
}): React.JSX.Element {
  return (
    <DropdownMenuItem
      className={cn(
        'h-8 rounded-[6px] px-2.5 text-[12.5px] font-medium',
        active ? 'bg-accent text-accent-foreground' : 'text-popover-foreground'
      )}
      onSelect={() => onClick?.()}
    >
      {children}
    </DropdownMenuItem>
  )
}
