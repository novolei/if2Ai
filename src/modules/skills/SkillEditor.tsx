//! SkillEditor — create/edit/patch UI with frontmatter validation.
//!
//! Provides a form interface for editing SKILL.md frontmatter with real-time validation.

import { useState, useCallback } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { AlertCircle, Plus, Trash2 } from 'lucide-react'
import type { SkillConfigVar, SkillFrontmatter } from './types'

interface SkillEditorProps {
  /** Initial frontmatter data */
  initialData?: Partial<SkillFrontmatter>
  /** Callback when save is requested */
  onSave: (data: SkillFrontmatter, content: string) => void
  /** Callback when cancel is requested */
  onCancel: () => void
  /** Skill name (for display, read-only) */
  skillName?: string
  /** Existing SKILL.md content */
  initialContent?: string
  /** Whether the editor is loading */
  loading?: boolean
  className?: string
}

interface ValidationError {
  field: string
  message: string
}

const PLATFORMS = ['macos', 'linux', 'windows', 'darwin', 'win32'] as const

export function SkillEditor({
  initialData = {},
  onSave,
  onCancel,
  skillName,
  initialContent = '',
  loading = false,
  className,
}: SkillEditorProps) {
  const [name, setName] = useState(initialData.name || skillName || '')
  const [description, setDescription] = useState(initialData.description || '')
  const [version, setVersion] = useState(initialData.version || '')
  const [license, setLicense] = useState(initialData.license || '')
  const [platforms, setPlatforms] = useState<string[]>(initialData.platforms || [])
  const [configVars, setConfigVars] = useState<SkillConfigVar[]>(
    initialData.config || initialData.hermes?.config || []
  )
  const [content, setContent] = useState(initialContent)
  const [errors, setErrors] = useState<ValidationError[]>([])
  const [touched, setTouched] = useState(false)

  const validate = useCallback((): ValidationError[] => {
    const errs: ValidationError[] = []

    if (!name.trim()) {
      errs.push({ field: 'name', message: '名称不能为空' })
    } else if (!/^[a-z0-9-]+$/.test(name)) {
      errs.push({ field: 'name', message: '名称只能包含小写字母、数字和连字符' })
    } else if (name.length > 64) {
      errs.push({ field: 'name', message: '名称不能超过 64 个字符' })
    }

    if (description.length > 1024) {
      errs.push({ field: 'description', message: '描述不能超过 1024 个字符' })
    }

    for (const platform of platforms) {
      if (!PLATFORMS.includes(platform as typeof PLATFORMS[number])) {
        errs.push({ field: 'platforms', message: `不支持的平台: ${platform}` })
      }
    }

    for (const configVar of configVars) {
      if (!configVar.key.trim()) {
        errs.push({ field: `config.${configVar.key}`, message: '配置变量 key 不能为空' })
      }
      if (configVar.description.length > 256) {
        errs.push({
          field: `config.${configVar.key}`,
          message: '配置变量描述不能超过 256 个字符',
        })
      }
    }

    return errs
  }, [name, description, platforms, configVars])

  const handleSave = () => {
    setTouched(true)
    const validationErrors = validate()
    setErrors(validationErrors)

    if (validationErrors.length > 0) {
      return
    }

    const frontmatter: SkillFrontmatter = {
      name: name.trim(),
      description: description.trim(),
      version: version.trim() || undefined,
      license: license.trim() || undefined,
      platforms: platforms.length > 0 ? platforms : undefined,
      config: configVars.length > 0 ? configVars : undefined,
    }

    onSave(frontmatter, content)
  }

  const addConfigVar = () => {
    setConfigVars([...configVars, { key: '', description: '', default: undefined }])
  }

  const updateConfigVar = (index: number, field: keyof SkillConfigVar, value: string) => {
    const updated = [...configVars]
    updated[index] = { ...updated[index], [field]: value }
    setConfigVars(updated)
  }

  const removeConfigVar = (index: number) => {
    setConfigVars(configVars.filter((_, i) => i !== index))
  }

  const togglePlatform = (platform: string) => {
    if (platforms.includes(platform)) {
      setPlatforms(platforms.filter((p) => p !== platform))
    } else {
      setPlatforms([...platforms, platform])
    }
  }

  const getFieldError = (field: string): string | undefined => {
    if (!touched) return undefined
    return errors.find((e) => e.field === field)?.message
  }

  const hasErrors = touched && errors.length > 0

  return (
    <div className={cn('space-y-6', className)}>
      {/* Header */}
      <div>
        <h3 className="text-lg font-semibold">
          {skillName ? `编辑技能: ${skillName}` : '创建新技能'}
        </h3>
        <p className="mt-1 text-sm text-muted-foreground">
          填写技能元数据，系统将生成标准化的 SKILL.md 文件
        </p>
      </div>

      {/* Validation Errors */}
      {hasErrors && (
        <div className="rounded-lg border border-red-200 bg-red-50 p-4">
          <div className="flex items-center gap-2 text-red-600">
            <AlertCircle className="h-4 w-4" />
            <span className="font-medium">验证错误</span>
          </div>
          <ul className="list-disc space-y-1 pl-6 mt-2">
            {errors.map((err, idx) => (
              <li key={idx} className="text-sm text-red-600">
                {err.message}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Basic Info */}
      <div className="space-y-4">
        <h4 className="text-sm font-medium">基本信息</h4>

        <div className="space-y-2">
          <label className="text-sm font-medium">名称 *</label>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="my-skill-name"
            className={cn(getFieldError('name') && 'border-red-500')}
          />
          {getFieldError('name') && (
            <p className="text-xs text-red-500">{getFieldError('name')}</p>
          )}
          <p className="text-xs text-muted-foreground">小写字母、数字和连字符，64 字符以内</p>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">描述 *</label>
          <Textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="简要描述技能的功能..."
            rows={3}
            className={cn(getFieldError('description') && 'border-red-500')}
          />
          {getFieldError('description') && (
            <p className="text-xs text-red-500">{getFieldError('description')}</p>
          )}
          <p className="text-xs text-muted-foreground">1024 字符以内</p>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">版本</label>
            <Input
              value={version}
              onChange={(e) => setVersion(e.target.value)}
              placeholder="1.0.0"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">许可证</label>
            <Input value={license} onChange={(e) => setLicense(e.target.value)} placeholder="MIT" />
          </div>
        </div>
      </div>

      {/* Platforms */}
      <div className="space-y-3">
        <h4 className="text-sm font-medium">支持平台</h4>
        <div className="flex flex-wrap gap-2">
          {['macos', 'linux', 'windows'].map((platform) => (
            <button
              key={platform}
              type="button"
              onClick={() => togglePlatform(platform)}
              className={cn(
                'rounded-full border px-3 py-1.5 text-sm transition-colors',
                platforms.includes(platform)
                  ? 'border-black/90 bg-black/90 text-white'
                  : 'border-black/10 bg-white hover:bg-black/[0.04]'
              )}
            >
              {platform}
            </button>
          ))}
        </div>
      </div>

      {/* Config Variables */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h4 className="text-sm font-medium">配置变量</h4>
          <Button type="button" variant="outline" size="sm" onClick={addConfigVar}>
            <Plus className="mr-1 h-4 w-4" />
            添加变量
          </Button>
        </div>

        {configVars.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            暂无配置变量。点击"添加变量"定义技能需要的配置项。
          </p>
        ) : (
          <div className="space-y-3">
            {configVars.map((configVar, idx) => (
              <div
                key={idx}
                className="rounded-lg border border-black/8 bg-white/80 p-4 space-y-3"
              >
                <div className="flex items-center justify-between">
                  <Badge variant="outline">变量 {idx + 1}</Badge>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => removeConfigVar(idx)}
                    className="text-muted-foreground hover:text-red-500"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
                <div className="grid grid-cols-3 gap-3">
                  <div className="space-y-1">
                    <label className="text-xs font-medium">Key</label>
                    <Input
                      value={configVar.key}
                      onChange={(e) => updateConfigVar(idx, 'key', e.target.value)}
                      placeholder="API_KEY"
                      className="font-mono"
                    />
                  </div>
                  <div className="col-span-2 space-y-1">
                    <label className="text-xs font-medium">描述</label>
                    <Input
                      value={configVar.description}
                      onChange={(e) => updateConfigVar(idx, 'description', e.target.value)}
                      placeholder="用途说明..."
                    />
                  </div>
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium">默认值（可选）</label>
                  <Input
                    value={configVar.default || ''}
                    onChange={(e) => updateConfigVar(idx, 'default', e.target.value)}
                    placeholder="留空则无默认值"
                    className="font-mono"
                  />
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Skill Content */}
      <div className="space-y-2">
        <h4 className="text-sm font-medium">技能内容（SKILL.md 正文）</h4>
        <Textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="# 技能标题&#10;&#10;在这里描述技能的具体功能和用法..."
          rows={12}
          className="font-mono text-sm"
        />
        <p className="text-xs text-muted-foreground">
          技能的具体实现内容，将追加到 frontmatter 之后
        </p>
      </div>

      {/* Preview */}
      {name && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium">Frontmatter 预览</h4>
          <pre className="rounded-lg border border-black/8 bg-black/[0.02] p-4 text-xs font-mono overflow-auto">
{`---
name: ${name}
description: ${description || '...'}
${version ? `version: ${version}` : ''}
${license ? `license: ${license}` : ''}
${platforms.length > 0 ? `platforms: [${platforms.join(', ')}]` : ''}
${configVars.length > 0 ? `config:` : ''}
${configVars.map((v) => `  - key: ${v.key}
    description: ${v.description}
    default: "${v.default || ''}"`).join('\n')}
---`}
          </pre>
        </div>
      )}

      {/* Actions */}
      <div className="flex justify-end gap-3 pt-4 border-t">
        <Button type="button" variant="outline" onClick={onCancel} disabled={loading}>
          取消
        </Button>
        <Button onClick={handleSave} disabled={loading}>
          {loading ? '保存中...' : '保存'}
        </Button>
      </div>
    </div>
  )
}

export default SkillEditor
