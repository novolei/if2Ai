import { useState } from 'react';
import { ChevronRight, ChevronDown, Plus, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { ProjectMeta } from '@/lib/tauri';
import type { SessionMeta } from '@/lib/tauri';

export interface ProjectRailProps {
  /** Project list */
  projects: ProjectMeta[];
  /** Sessions for each project (keyed by project ID) */
  projectSessions: Record<string, SessionMeta[]>;
  /** Currently active project ID */
  activeProjectId: string | null;
  /** Currently active session ID */
  activeSessionId: string | null;
  /** Called when a project is selected */
  onSelectProject: (id: string) => void;
  /** Called when a session is selected */
  onSelectSession: (projectId: string, sessionId: string) => void;
  /** Called when "New Chat" is clicked for a project */
  onNewChat: (projectId: string) => void;
  /** Called when delete project is clicked */
  onDeleteProject: (id: string) => void;
  /** Called when delete session is clicked */
  onDeleteSession: (projectId: string, sessionId: string) => void;
  /** Loading state */
  loading?: boolean;
}

interface ProjectItemProps {
  project: ProjectMeta;
  sessions: SessionMeta[];
  isExpanded: boolean;
  isActive: boolean;
  activeSessionId: string | null;
  onToggle: () => void;
  onSelectProject: () => void;
  onSelectSession: (sessionId: string) => void;
  onNewChat: () => void;
  onDeleteProject: () => void;
  onDeleteSession: (sessionId: string) => void;
}

function ProjectItem({
  project,
  sessions,
  isExpanded,
  isActive,
  activeSessionId,
  onToggle,
  onSelectProject,
  onSelectSession,
  onNewChat,
  onDeleteProject,
  onDeleteSession,
}: ProjectItemProps) {
  return (
    <div className="select-none">
      {/* Project row */}
      <div
        className={cn(
          'group flex items-center gap-1 px-2 py-1 rounded-md cursor-pointer text-sm',
          isActive ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'
        )}
        onClick={onSelectProject}
      >
        <button
          onClick={(e) => {
            e.stopPropagation();
            onToggle();
          }}
          className="p-0.5 hover:bg-accent rounded"
        >
          {isExpanded ? (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          )}
        </button>
        <span className="flex-1 truncate">{project.name}</span>
        <span className="text-xs text-muted-foreground">
          {project.session_count}
        </span>
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 opacity-0 group-hover:opacity-100"
          onClick={(e) => {
            e.stopPropagation();
            onDeleteProject();
          }}
        >
          <Trash2 className="h-3 w-3 text-muted-foreground" />
        </Button>
      </div>

      {/* Sessions */}
      {isExpanded && (
        <div className="ml-6 mt-1 space-y-1">
          {sessions.map((session) => (
            <div
              key={session.id}
              className={cn(
                'group flex items-center gap-2 px-2 py-1 rounded-md cursor-pointer text-sm',
                activeSessionId === session.id
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-accent/50'
              )}
              onClick={() => onSelectSession(session.id)}
            >
              <span className="flex-1 truncate">{session.title}</span>
              <Button
                variant="ghost"
                size="icon"
                className="h-5 w-5 opacity-0 group-hover:opacity-100"
                onClick={(e) => {
                  e.stopPropagation();
                  onDeleteSession(session.id);
                }}
              >
                <Trash2 className="h-3 w-3 text-muted-foreground" />
              </Button>
            </div>
          ))}
          {/* New Chat button */}
          <button
            onClick={(e) => {
              e.stopPropagation();
              onNewChat();
            }}
            className="flex items-center gap-2 px-2 py-1 rounded-md text-sm text-muted-foreground hover:text-foreground hover:bg-accent/50 w-full"
          >
            <Plus className="h-3 w-3" />
            <span>新建对话</span>
          </button>
        </div>
      )}
    </div>
  );
}

export function ProjectRail({
  projects,
  projectSessions,
  activeProjectId,
  activeSessionId,
  onSelectProject,
  onSelectSession,
  onNewChat,
  onDeleteProject,
  onDeleteSession,
  loading = false,
}: ProjectRailProps) {
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(
    new Set(activeProjectId ? [activeProjectId] : [])
  );

  const toggleExpanded = (projectId: string) => {
    setExpandedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
      }
      return next;
    });
  };

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b">
        <h2 className="text-sm font-semibold">项目</h2>
      </div>
      <div className="flex-1 overflow-y-auto p-2">
        {loading ? (
          <div className="text-sm text-muted-foreground p-2">加载中...</div>
        ) : projects.length === 0 ? (
          <div className="text-sm text-muted-foreground p-2">暂无项目</div>
        ) : (
          <div className="space-y-2">
            {projects.map((project) => (
              <ProjectItem
                key={project.id}
                project={project}
                sessions={projectSessions[project.id] || []}
                isExpanded={expandedProjects.has(project.id)}
                isActive={activeProjectId === project.id}
                activeSessionId={activeSessionId}
                onToggle={() => toggleExpanded(project.id)}
                onSelectProject={() => {
                  onSelectProject(project.id);
                  setExpandedProjects((prev) => new Set([...prev, project.id]));
                }}
                onSelectSession={(sessionId) =>
                  onSelectSession(project.id, sessionId)
                }
                onNewChat={() => onNewChat(project.id)}
                onDeleteProject={() => onDeleteProject(project.id)}
                onDeleteSession={(sessionId) =>
                  onDeleteSession(project.id, sessionId)
                }
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
