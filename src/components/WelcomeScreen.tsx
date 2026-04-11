import { useState } from 'react';
import { FolderPlus, MessageSquare, ChevronDown } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { Project, ProjectMeta } from '@/lib/tauri';

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
    <div className="flex flex-col items-center justify-center h-full p-8">
      <div className="text-center space-y-6 max-w-md">
        {/* Header */}
        <div className="space-y-2">
          <h1 className="text-3xl font-bold tracking-tight">
            有什么我可以帮助你的？
          </h1>
          <p className="text-muted-foreground">
            选择一个项目开始新对话，或创建新项目
          </p>
        </div>

        {/* Project selector */}
        <div className="relative">
          <Button
            variant="outline"
            className="w-full justify-between"
            onClick={() => setProjectDropdownOpen(!projectDropdownOpen)}
            disabled={loading}
          >
            <span className="truncate">
              {currentProject ? currentProject.name : '选择项目'}
            </span>
            <ChevronDown
              className={cn(
                'h-4 w-4 text-muted-foreground transition-transform',
                projectDropdownOpen && 'rotate-180'
              )}
            />
          </Button>

          {/* Dropdown menu */}
          {projectDropdownOpen && (
            <div className="absolute top-full left-0 right-0 mt-1 py-1 bg-background border rounded-md shadow-lg z-10">
              {projects.length === 0 ? (
                <div className="px-3 py-2 text-sm text-muted-foreground">
                  暂无项目
                </div>
              ) : (
                projects.map((project) => (
                  <button
                    key={project.id}
                    className={cn(
                      'w-full px-3 py-2 text-left text-sm hover:bg-accent',
                      currentProject?.id === project.id && 'bg-accent'
                    )}
                    onClick={() => {
                      onSelectProject(project.id);
                      setProjectDropdownOpen(false);
                    }}
                  >
                    <div className="font-medium truncate">{project.name}</div>
                    <div className="text-xs text-muted-foreground truncate">
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
          <Button
            variant="outline"
            className="gap-2"
            onClick={onCreateProject}
            disabled={loading}
          >
            <FolderPlus className="h-4 w-4" />
            <span>新建项目</span>
          </Button>

          {/* Start new chat (only if project selected) */}
          {currentProject && (
            <Button
              className="gap-2"
              onClick={() => onStartNewChat(currentProject.id)}
              disabled={loading}
            >
              <MessageSquare className="h-4 w-4" />
              <span>在 &ldquo;{currentProject.name}&rdquo; 中开始新对话</span>
            </Button>
          )}
        </div>

        {/* No projects state */}
        {projects.length === 0 && !loading && (
          <div className="text-sm text-muted-foreground">
            创建第一个项目开始使用
          </div>
        )}
      </div>
    </div>
  );
}
