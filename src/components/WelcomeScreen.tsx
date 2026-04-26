import { FolderPlus, MessageSquare, FolderOpen, Sparkles, ArrowRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import appIconUrl from '@/assets/app-icon.png'
import type { Project, ProjectMeta } from '@/lib/tauri'
import type { ReactNode } from 'react'

export interface WelcomeScreenProps {
  currentProject: Project | null
  projects: ProjectMeta[]
  onSelectProject: (id: string) => void
  onCreateProject: () => void
  onStartNewChat: (projectId: string) => void
  loading?: boolean
}

export function WelcomeScreen({
  currentProject,
  projects,
  onSelectProject,
  onCreateProject,
  onStartNewChat,
  loading = false,
}: WelcomeScreenProps) {
  return (
    <div className="flex h-full min-h-0 items-center justify-center bg-[radial-gradient(circle_at_top,rgba(153,183,169,0.16),transparent_32%),radial-gradient(circle_at_right,rgba(132,154,207,0.14),transparent_30%),linear-gradient(180deg,rgba(249,250,251,0.8),rgba(244,246,248,1))] p-4 lg:p-8">
      <div className="grid w-full max-w-6xl gap-6 xl:grid-cols-[0.95fr_1.05fr]">
        <Card className="border-border/70 bg-background/90 shadow-xl shadow-slate-900/5 backdrop-blur">
          <CardContent className="flex h-full flex-col items-center justify-center gap-8 p-8 text-center lg:p-12">
            <div className="relative">
              <div className="absolute inset-[-36px] rounded-[42px] bg-[radial-gradient(circle_at_50%_50%,rgba(239,68,38,0.22),transparent_68%)] blur-2xl" />
              <div className="relative size-[136px] overflow-hidden rounded-[32px] shadow-[0_1px_0_0.5px_rgba(255,255,255,0.68),0_0_0_0.5px_rgba(0,0,0,0.12),0_16px_36px_rgba(239,68,38,0.20),0_28px_64px_rgba(15,23,42,0.10)]">
                <img
                  src={appIconUrl}
                  alt="If2Ai app icon"
                  className="size-full object-cover"
                  draggable={false}
                />
                <div className="pointer-events-none absolute inset-0 rounded-[32px] bg-[linear-gradient(145deg,rgba(255,255,255,0.16)_0%,rgba(255,255,255,0.05)_38%,transparent_62%)]" />
              </div>
            </div>

            <div className="max-w-xl space-y-4">
              <Badge variant="secondary" className="gap-2 rounded-full px-3 py-1.5">
                <Sparkles className="h-3.5 w-3.5" />
                Desktop Agent Workspace
              </Badge>
              <h1 className="text-4xl font-semibold tracking-tight lg:text-5xl">
                有什么我可以帮助你的？
              </h1>
              <p className="text-base leading-7 text-muted-foreground lg:text-lg">
                选择一个项目开始新对话，或者直接创建一个新的工作区。整个界面围绕任务流、项目组织和高质感桌面体验来设计。
              </p>
            </div>

            <div className="flex flex-wrap justify-center gap-3">
              <Button onClick={onCreateProject} className="gap-2 rounded-2xl px-5">
                <FolderPlus className="h-4 w-4" />
                新建项目
              </Button>
              {currentProject && (
                <Button
                  variant="outline"
                  onClick={() => onStartNewChat(currentProject.id)}
                  className="gap-2 rounded-2xl px-5"
                >
                  <MessageSquare className="h-4 w-4" />
                  开始新对话
                </Button>
              )}
            </div>

            <div className="grid w-full gap-3 sm:grid-cols-2">
              <FeaturePill title="项目优先" text="先选工作区，再展开智能体对话。">
                <FolderOpen className="h-4 w-4" />
              </FeaturePill>
              <FeaturePill title="流式输出" text="响应内容实时呈现，思考过程可展开查看。">
                <ArrowRight className="h-4 w-4" />
              </FeaturePill>
            </div>
          </CardContent>
        </Card>

        <Card className="border-border/70 bg-background/90 shadow-xl shadow-slate-900/5 backdrop-blur">
          <CardHeader>
            <CardTitle>快速开始</CardTitle>
            <CardDescription>
              先选一个项目，If2Ai 会自动恢复最近一次的工作状态。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="grid gap-2">
              <div className="text-sm font-medium">当前项目</div>
              {projects.length === 0 ? (
                <Button variant="outline" disabled className="h-12 justify-start rounded-2xl">
                  暂无项目
                </Button>
              ) : (
                <Select
                  value={currentProject?.id ?? ''}
                  onValueChange={(value) => onSelectProject(value)}
                  disabled={loading}
                >
                  <SelectTrigger className="h-12 rounded-2xl">
                    <SelectValue placeholder="选择项目" />
                  </SelectTrigger>
                  <SelectContent>
                    {projects.map((project) => (
                      <SelectItem key={project.id} value={project.id}>
                        <div className="flex flex-col py-1">
                          <span className="font-medium">{project.name}</span>
                          <span className="text-xs text-muted-foreground">{project.workdir}</span>
                        </div>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </div>

            <div className="rounded-3xl border border-border/70 bg-muted/20 p-4">
              <div className="text-sm font-medium">最近项目</div>
              <div className="mt-3 space-y-2">
                {projects.length === 0 ? (
                  <div className="rounded-2xl border border-dashed border-border/60 bg-background px-4 py-6 text-center text-sm text-muted-foreground">
                    暂时没有可用项目
                  </div>
                ) : (
                  projects.slice(0, 4).map((project) => (
                    <button
                      key={project.id}
                      type="button"
                      onClick={() => onSelectProject(project.id)}
                      className="flex w-full items-center justify-between rounded-2xl border border-transparent bg-background px-4 py-3 text-left transition-colors hover:border-border/70 hover:bg-muted/30"
                    >
                      <div className="min-w-0">
                        <div className="truncate text-sm font-medium">{project.name}</div>
                        <div className="truncate text-xs text-muted-foreground">{project.workdir}</div>
                      </div>
                      <Badge variant="secondary" className="rounded-full">
                        {project.session_count}
                      </Badge>
                    </button>
                  ))
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

function FeaturePill({
  title,
  text,
  children,
}: {
  title: string
  text: string
  children: ReactNode
}) {
  return (
    <div className="flex items-start gap-3 rounded-3xl border border-border/70 bg-muted/20 p-4 text-left">
      <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-primary/10 text-primary">
        {children}
      </div>
      <div>
        <div className="text-sm font-medium">{title}</div>
        <div className="mt-1 text-sm leading-6 text-muted-foreground">{text}</div>
      </div>
    </div>
  )
}
