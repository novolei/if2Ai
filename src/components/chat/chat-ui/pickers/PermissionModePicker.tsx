/**
 * GF-01 PR-06 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Permission-mode dropdown rendered inline in the composer's bottom-bar
 * left cluster. Render-equivalent move from the legacy inline JSX —
 * trigger label, dropdown alignment, items list and click handler are
 * preserved verbatim.
 */

import * as React from "react"
import { ChevronDown } from "lucide-react"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { MenuItemButton } from "@/components/chat/chat-ui/utils/MenuItemButton"
import {
  permissionModeItems,
  permissionModeLabelFor,
} from "@/components/chat/chat-ui/utils/items"
import type { PermissionMode } from "@/lib/tauri"

/** Props for {@link PermissionModePicker} — mirrors the legacy inline render. */
export interface PermissionModePickerProps {
  /** Currently active permission mode value. */
  permissionMode: PermissionMode
  /** Setter that switches the active permission mode. */
  setPermissionMode: React.Dispatch<React.SetStateAction<PermissionMode>>
  /** Disables the trigger while a turn is in flight. */
  disabled?: boolean
}

/**
 * Dropdown trigger + menu used by the composer to switch permission
 * mode (`dangerFullAccess` / `workspaceWrite` / `readOnly`).
 */
export function PermissionModePicker({
  permissionMode,
  setPermissionMode,
  disabled,
}: PermissionModePickerProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          disabled={disabled}
        >
          {permissionModeLabelFor(permissionMode)}
          <ChevronDown className="h-3 w-3" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent sideOffset={6} align="start" className="w-[148px]">
        {permissionModeItems.map((item) => (
          <MenuItemButton
            key={item.value}
            active={permissionMode === item.value}
            onClick={() => setPermissionMode(item.value)}
          >
            {item.label}
          </MenuItemButton>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
