import { useEffect, useState, type FormEvent } from 'react'
import { FolderOpen, FolderPlus } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

interface CreateProjectDialogProps {
  isOpen: boolean
  onClose: () => void
  onSubmit: (name: string, workdir: string) => Promise<void>
}

export function CreateProjectDialog({
  isOpen,
  onClose,
  onSubmit,
}: CreateProjectDialogProps) {
  const [name, setName] = useState('')
  const [workdir, setWorkdir] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    if (!isOpen) {
      return
    }

    setName('')
    setWorkdir('')
    setIsSubmitting(false)
  }, [isOpen])

  const handleSelectDirectory = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择项目目录',
      })

      if (selected && typeof selected === 'string') {
        setWorkdir(selected)
        if (!name.trim()) {
          const folderName = selected.split(/[\\/]/).filter(Boolean).pop()
          if (folderName) {
            setName(folderName)
          }
        }
      }
    } catch (err) {
      console.error('Failed to select directory:', err)
    }
  }

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (!name.trim() || !workdir.trim()) return

    setIsSubmitting(true)
    try {
      await onSubmit(name.trim(), workdir.trim())
      onClose()
    } catch (err) {
      console.error('Failed to create project:', err)
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={(openState) => !openState && onClose()}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FolderPlus className="h-5 w-5 text-primary" />
            新建项目
          </DialogTitle>
          <DialogDescription>
            选择一个本地目录，为它创建一个新的智能体工作区和首个会话。
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="grid gap-5">
          <div className="grid gap-2">
            <Label htmlFor="project-name">项目名称</Label>
            <Input
              id="project-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如：产品重构"
              autoFocus
            />
          </div>

          <div className="grid gap-2">
            <Label htmlFor="project-workdir">工作目录</Label>
            <div className="flex gap-2">
              <Input
                id="project-workdir"
                value={workdir}
                onChange={(e) => setWorkdir(e.target.value)}
                placeholder="选择或输入目录路径"
                className="flex-1"
              />
              <Button type="button" variant="outline" size="icon" onClick={handleSelectDirectory}>
                <FolderOpen className="h-4 w-4" />
              </Button>
            </div>
          </div>

          <div className="rounded-2xl border border-border/60 bg-muted/20 p-4 text-sm text-muted-foreground">
            建议为每个项目使用独立目录，方便后续的会话、配置和文件操作保持清晰。
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={isSubmitting}>
              取消
            </Button>
            <Button type="submit" disabled={!name.trim() || !workdir.trim() || isSubmitting}>
              {isSubmitting ? '创建中...' : '创建'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
