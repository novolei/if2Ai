import { BrowserRouter, Routes, Route } from "react-router-dom";
import React from "react";
import { Toaster, toast } from "sonner";
import TokenSystem from "./pages/TokenSystem";
import ChatWorkspace from "./pages/ChatWorkspace";
import ComponentSystem from "./pages/ComponentSystem";
import NotFound from "./pages/NotFound";

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
    toast.error("Something went wrong, please refresh the page");
  }
  render() {
    return this.state.hasError ? (
      <div className="p-8 text-center text-muted-foreground">Something went wrong</div>
    ) : (
      this.props.children
    );
  }
}

const App = () => (
  <BrowserRouter>
    <ErrorBoundary>
      <Routes>
        <Route path="/" element={<ChatWorkspace />} />
        <Route path="/token-system" element={<TokenSystem />} />
        <Route path="/component-system" element={<ComponentSystem />} />
        <Route path="*" element={<NotFound />} />
      </Routes>
    </ErrorBoundary>
    <Toaster position="top-right" richColors />
  </BrowserRouter>
);

export default App;
