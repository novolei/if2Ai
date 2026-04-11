import { useState, useRef, useEffect } from 'react';
import { ChevronRight, ChevronDown, Plus, Trash2, MoreHorizontal, Pencil, Copy } from 'lucide-react';
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
  /** Called when rename project is clicked */
  onRenameProject: (id: string, newName: string) => void;
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
  onRenameProject: (newName: string) => void;
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
  onRenameProject,
  onDeleteSession,
}: ProjectItemProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
  const [renaming, setRenaming] = useState(false);
  const [newName, setNewName] = useState(project.name);
  const menuRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  // Close menu when clicking outside
  useEffect(() => {
    if (!menuOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [menuOpen]);

  // Focus input when renaming starts
  useEffect(() => {
    if (renaming && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [renaming]);

  const handleCopyPath = () => {
    navigator.clipboard.writeText(project.workdir);
    setMenuOpen(false);
  };

  const handleRename = () => {
    if (newName.trim() && newName !== project.name) {
      onRenameProject(newName.trim());
    }
    setRenaming(false);
    setMenuOpen(false);
  };

  return (
    <div className="select-none">
      {/* Project row */}
      <div
        className={cn(
          'group flex items-center gap-1 px-2 py-1 rounded-md cursor-pointer text-sm',
          isActive ? 'bg-accent text-accent-foreground' : 'hover:bg-gray-100'
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

        {renaming ? (
          <input
            ref={inputRef}
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleRename();
              if (e.key === 'Escape') {
                setRenaming(false);
                setNewName(project.name);
              }
            }}
            onBlur={handleRename}
            onClick={(e) => e.stopPropagation()}
            className="flex-1 px-1 py-0.5 text-sm bg-background border border-input rounded"
          />
        ) : (
          <span className="flex-1 truncate">{project.name}</span>
        )}

        {/* New chat button */}
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 opacity-0 group-hover:opacity-100"
          onClick={(e) => {
            e.stopPropagation();
            onNewChat();
          }}
          title="新建对话"
        >
          <Plus className="h-3 w-3 text-muted-foreground" />
        </Button>

        {/* Three-dot menu */}
        <div ref={menuRef} className="relative">
          <Button
            ref={buttonRef}
            variant="ghost"
            size="icon"
            className="h-6 w-6 opacity-0 group-hover:opacity-100"
            onClick={(e) => {
              e.stopPropagation();
              const rect = e.currentTarget.getBoundingClientRect();
              setMenuPosition({ x: rect.right, y: rect.top });
              setMenuOpen(!menuOpen);
            }}
          >
            <MoreHorizontal className="h-3 w-3 text-muted-foreground" />
          </Button>

          {menuOpen && (
            <div
              className="fixed bg-white border border-gray-200 rounded-lg shadow-xl z-[100] min-w-[160px]"
              style={{
                left: `${menuPosition.x}px`,
                top: `${menuPosition.y}px`,
              }}
            >
              <button
                className="flex items-center gap-2 w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100 active:bg-gray-200 transition-colors cursor-pointer first:rounded-t-lg last:rounded-b-lg"
                onClick={(e) => {
                  e.stopPropagation();
                  setMenuOpen(false);
                  setRenaming(true);
                }}
              >
                <Pencil className="h-3.5 w-3.5 text-gray-500 shrink-0" />
                修改名称
              </button>
              <button
                className="flex items-center gap-2 w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100 active:bg-gray-200 transition-colors cursor-pointer first:rounded-t-lg last:rounded-b-lg"
                onClick={(e) => {
                  e.stopPropagation();
                  handleCopyPath();
                }}
              >
                <Copy className="h-3.5 w-3.5 text-gray-500 shrink-0" />
                复制路径
              </button>
              <div className="h-px bg-gray-200" />
              <button
                className="flex items-center gap-2 w-full px-3 py-2 text-sm text-red-600 hover:bg-red-50 active:bg-red-100 transition-colors cursor-pointer first:rounded-t-lg last:rounded-b-lg"
                onClick={(e) => {
                  e.stopPropagation();
                  setMenuOpen(false);
                  onDeleteProject();
                }}
              >
                <Trash2 className="h-3.5 w-3.5 shrink-0" />
                删除项目
              </button>
            </div>
          )}
        </div>
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
                  ? 'bg-violet-100 text-violet-900'
                  : 'hover:bg-gray-100'
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
  onRenameProject,
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
                onRenameProject={(newName) => onRenameProject(project.id, newName)}
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
