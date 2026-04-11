import { useState } from 'react';
import { FolderPlus, MessageSquare, ChevronDown, Plus } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Project, ProjectMeta } from '@/lib/tauri';
import { AgentOrb } from './AgentOrb';

export interface WelcomeScreenProps {
  /** Current active project */
  currentProject: Project | null;
  /** Available projects */
  projects: ProjectMeta[];
  /** Called when a project is selected from dropdown */
  onSelectProject: (id: string) => void;
  /** Called when create project is clicked */
  onCreateProject: () => void;
  /** Called when start new chat is clicked */
  onStartNewChat: (projectId: string) => void;
  /** Loading state */
  loading?: boolean;
}

export function WelcomeScreen({
  currentProject,
  projects,
  onSelectProject,
  onCreateProject,
  onStartNewChat,
  loading = false,
}: WelcomeScreenProps) {
  const [projectDropdownOpen, setProjectDropdownOpen] = useState(false);

  return (
    <div
      className="flex flex-col items-center justify-center h-full p-8"
      style={{ backgroundColor: 'var(--color-bg-app)' }}
    >
      {/* Hero Orb */}
      <div className="mb-8">
        <AgentOrb status="idle" size="hero" />
      </div>

      <div className="text-center space-y-6 max-w-md">
        {/* Header */}
        <div className="space-y-2">
          <h1
            className="text-3xl font-bold tracking-tight"
            style={{ color: 'var(--color-text-primary)', fontFamily: 'var(--font-serif)' }}
          >
            有什么我可以帮助你的？
          </h1>
          <p style={{ color: 'var(--color-text-tertiary)' }}>
            选择一个项目开始新对话，或创建新项目
          </p>
        </div>

        {/* Project selector */}
        <div className="relative">
          <button
            className="w-full flex items-center justify-between px-4 py-3 rounded-xl text-sm transition-all"
            style={{
              backgroundColor: 'rgba(255, 255, 255, 0.7)',
              border: '1px solid var(--color-border-soft)',
              color: 'var(--color-text-primary)',
            }}
            onClick={() => setProjectDropdownOpen(!projectDropdownOpen)}
            disabled={loading}
            onMouseEnter={(e) => {
              e.currentTarget.style.borderColor = 'var(--color-border-strong)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.borderColor = 'var(--color-border-soft)'
            }}
          >
            <span className="truncate">
              {currentProject ? currentProject.name : '选择项目'}
            </span>
            <ChevronDown
              className={cn(
                'h-4 w-4 transition-transform',
                projectDropdownOpen && 'rotate-180'
              )}
              style={{ color: 'var(--color-text-secondary)' }}
            />
          </button>

          {/* Dropdown menu */}
          {projectDropdownOpen && (
            <div
              className="absolute top-full left-0 right-0 mt-1 py-1 rounded-xl shadow-lg z-10"
              style={{
                backgroundColor: 'rgba(255, 255, 255, 0.95)',
                border: '1px solid var(--color-border-soft)',
                backdropFilter: 'blur(20px)',
              }}
            >
              {projects.length === 0 ? (
                <div
                  className="px-3 py-2 text-sm"
                  style={{ color: 'var(--color-text-tertiary)' }}
                >
                  暂无项目
                </div>
              ) : (
                projects.map((project) => (
                  <button
                    key={project.id}
                    className="w-full px-3 py-2 text-left text-sm transition-colors"
                    style={{
                      backgroundColor: currentProject?.id === project.id ? 'var(--color-primary-soft)' : 'transparent',
                      color: 'var(--color-text-primary)',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.backgroundColor = 'var(--color-primary-soft)'
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.backgroundColor = currentProject?.id === project.id ? 'var(--color-primary-soft)' : 'transparent'
                    }}
                    onClick={() => {
                      onSelectProject(project.id);
                      setProjectDropdownOpen(false);
                    }}
                  >
                    <div className="font-medium truncate">{project.name}</div>
                    <div
                      className="text-xs truncate"
                      style={{ color: 'var(--color-text-tertiary)' }}
                    >
                      {project.workdir}
                    </div>
                  </button>
                ))
              )}
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="flex flex-col gap-3">
          {/* Create new project */}
          <button
            className="w-full flex items-center justify-center gap-2 px-4 py-3 rounded-xl text-sm font-medium transition-all"
            style={{
              backgroundColor: 'rgba(255, 255, 255, 0.58)',
              border: '1px solid var(--color-border-soft)',
              color: 'var(--color-text-primary)',
            }}
            onClick={onCreateProject}
            disabled={loading}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.8)'
              e.currentTarget.style.borderColor = 'var(--color-border-strong)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.58)'
              e.currentTarget.style.borderColor = 'var(--color-border-soft)'
            }}
          >
            <FolderPlus className="h-4 w-4" />
            <span>新建项目</span>
          </button>

          {/* Start new chat (only if project selected) */}
          {currentProject && (
            <button
              className="w-full flex items-center justify-center gap-2 px-4 py-3 rounded-xl text-sm font-medium transition-all"
              style={{
                background: 'linear-gradient(135deg, var(--color-primary) 0%, var(--color-accent-mint) 100%)',
                color: 'var(--color-text-inverse)',
              }}
              onClick={() => onStartNewChat(currentProject.id)}
              disabled={loading}
              onMouseEnter={(e) => {
                e.currentTarget.style.filter = 'brightness(1.05)'
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.filter = 'brightness(1)'
              }}
            >
              <MessageSquare className="h-4 w-4" />
              <span>在 &ldquo;{currentProject.name}&rdquo; 中开始新对话</span>
            </button>
          )}
        </div>

        {/* No projects state */}
        {projects.length === 0 && !loading && (
          <div
            className="text-sm"
            style={{ color: 'var(--color-text-tertiary)' }}
          >
            创建第一个项目开始使用
          </div>
        )}
      </div>
    </div>
  );
}
