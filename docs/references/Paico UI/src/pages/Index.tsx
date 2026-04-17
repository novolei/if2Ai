import { useState } from "react";
import { Toaster, toast } from "sonner";
import React from "react";
import Sidebar from "../components/Sidebar";
import Topbar from "../components/Topbar";
import Dashboard from "./Dashboard";
import Projects from "./Projects";
import Moodboards from "./Moodboards";
import Materials from "./Materials";
import Proposals from "./Proposals";
import Timeline from "./Timeline";
import Clients from "./Clients";
import Files from "./Files";

type Section =
  | "dashboard"
  | "projects"
  | "moodboards"
  | "materials"
  | "proposals"
  | "timeline"
  | "clients"
  | "files";

const SECTION_TITLES: Record<Section, { title: string; subtitle: string }> = {
  dashboard: { title: "Studio Dashboard", subtitle: "Thursday, 12 June 2025" },
  projects: { title: "Project Management", subtitle: "All active & archived projects" },
  moodboards: { title: "Moodboards", subtitle: "Visual concept collections" },
  materials: { title: "Material Library", subtitle: "Textures, finishes & samples" },
  proposals: { title: "Proposal Builder", subtitle: "Client proposals & presentations" },
  timeline: { title: "Timeline & Gantt", subtitle: "Project scheduling overview" },
  clients: { title: "Client Management", subtitle: "Contacts, history & projects" },
  files: { title: "Files & Assets", subtitle: "Documents, drawings & media" },
};

class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  { hasError: boolean }
> {
  state = { hasError: false };
  static getDerivedStateFromError() {
    return { hasError: true };
  }
  componentDidCatch(error: Error) {
    console.log("ErrorBoundary caught:", error);
    toast.error("Something went wrong — please refresh the page.");
  }
  render() {
    return this.state.hasError ? (
      <div className="flex-1 flex items-center justify-center text-muted-foreground text-[13px] font-sans">
        Something went wrong. Please refresh.
      </div>
    ) : (
      this.props.children
    );
  }
}

export default function Index() {
  const [activeSection, setActiveSection] = useState<Section>("dashboard");

  const { title, subtitle } = SECTION_TITLES[activeSection];

  const handleNavigate = (id: string) => {
    console.log("Navigating to section:", id);
    setActiveSection(id as Section);
  };

  const renderSection = () => {
    switch (activeSection) {
      case "dashboard":
        return <Dashboard onNavigate={handleNavigate} />;
      case "projects":
        return <Projects />;
      case "moodboards":
        return <Moodboards />;
      case "materials":
        return <Materials />;
      case "proposals":
        return <Proposals />;
      case "timeline":
        return <Timeline />;
      case "clients":
        return <Clients />;
      case "files":
        return <Files />;
      default:
        return <Dashboard onNavigate={handleNavigate} />;
    }
  };

  return (
    <div data-cmp="Index" className="flex h-screen w-screen overflow-hidden bg-background" style={{ minWidth: "1440px" }}>
      <Sidebar activeSection={activeSection} onNavigate={handleNavigate} />

      <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
        <Topbar
          title={title}
          subtitle={subtitle}
          onNewProject={() => {
            console.log("New project clicked");
            toast.success("New project dialog coming soon.");
          }}
        />
        <main className="flex-1 overflow-hidden flex flex-col">
          <ErrorBoundary>{renderSection()}</ErrorBoundary>
        </main>
      </div>

      <Toaster position="top-right" richColors />
    </div>
  );
}
