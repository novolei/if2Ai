import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  usePermissionOverlay,
  type UsePermissionOverlayResult,
} from "./usePermissionOverlay";

/**
 * Optional pre-resolved hook result. When omitted the host calls
 * `usePermissionOverlay()` itself; tests / Storybook can inject a
 * fixture instead.
 */
export interface PermissionOverlayHostProps {
  overlay?: UsePermissionOverlayResult;
}

/**
 * Pure presentation component for the permission-request dialog.
 * Reads `{ permissionPrompt, decide }` from `usePermissionOverlay()`
 * (or an injected `overlay` prop) and renders the same Dialog that
 * previously lived inline in `App.tsx` (GF-03 PR-5).
 */
export function PermissionOverlayHost({
  overlay,
}: PermissionOverlayHostProps = {}): JSX.Element {
  const fallback = usePermissionOverlay();
  const { permissionPrompt, decide } = overlay ?? fallback;

  return (
    <Dialog open={Boolean(permissionPrompt)}>
      <DialogContent
        showCloseButton={false}
        onEscapeKeyDown={(e) => e.preventDefault()}
        onPointerDownOutside={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>权限请求</DialogTitle>
          <DialogDescription>
            {permissionPrompt?.message ?? "该操作需要更高权限。"}
          </DialogDescription>
        </DialogHeader>
        {permissionPrompt && (
          <div className="rounded-lg border border-black/10 bg-black/[0.02] px-3 py-2 text-[12px] text-black/60">
            工具：{permissionPrompt.tool_name} · 当前模式：
            {permissionPrompt.current_mode}
          </div>
        )}
        <DialogFooter className="sm:justify-between">
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => decide("deny", "once")}
            >
              拒绝本次
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => decide("deny", "session")}
            >
              本会话拒绝
            </Button>
          </div>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() => decide("allow", "once")}
            >
              允许本次
            </Button>
            <Button
              type="button"
              size="sm"
              onClick={() => decide("allow", "session")}
            >
              本会话允许
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
