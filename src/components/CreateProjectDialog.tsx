import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { FolderOpen, X } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'

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

  if (!isOpen) return null

  const handleSelectDirectory = async () => {
    console.log('[DEBUG] handleSelectDirectory called')
    try {
      console.log('[DEBUG] Opening dialog...')
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择项目目录',
      })
      console.log('[DEBUG] Dialog result:', selected)
      if (selected && typeof selected === 'string') {
        setWorkdir(selected)
      }
    } catch (err) {
      console.error('[DEBUG] Failed to select directory:', err)
    }
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim() || !workdir.trim()) return

    setIsSubmitting(true)
    try {
      await onSubmit(name.trim(), workdir.trim())
      setName('')
      setWorkdir('')
      onClose()
    } catch (err) {
      console.error('Failed to create project:', err)
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Dialog */}
      <div className="relative bg-background rounded-lg shadow-xl border border-border w-full max-w-md mx-4 p-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">新建项目</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-accent rounded-md transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="project-name" className="block text-sm font-medium mb-1.5">
              项目名称
            </label>
            <Input
              id="project-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如：我的项目"
              autoFocus
            />
          </div>

          <div>
            <label htmlFor="project-workdir" className="block text-sm font-medium mb-1.5">
              工作目录
            </label>
            <div className="flex gap-2">
              <Input
                id="project-workdir"
                value={workdir}
                onChange={(e) => setWorkdir(e.target.value)}
                placeholder="选择或输入目录路径"
                className="flex-1"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                onClick={handleSelectDirectory}
                title="选择目录"
              >
                <FolderOpen className="h-4 w-4" />
              </Button>
            </div>
          </div>

          <div className="flex justify-end gap-2 pt-4">
            <Button type="button" variant="outline" onClick={onClose}>
              取消
            </Button>
            <Button type="submit" disabled={!name.trim() || !workdir.trim() || isSubmitting}>
              {isSubmitting ? '创建中...' : '创建'}
            </Button>
          </div>
        </form>
      </div>
    </div>
  )
}
