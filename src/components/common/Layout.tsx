import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { LibraryList } from "../library/LibraryList";
import { useState } from "react";

export function Layout() {
  const [sidebarWidth, setSidebarWidth] = useState(220);
  const [listWidth, setListWidth] = useState(300);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background">
      {/* macOS titlebar drag region */}
      <div className="drag-region fixed top-0 left-0 right-0 h-8 z-50 pointer-events-none" />

      {/* Pane 1: Sidebar */}
      <aside
        className="flex-none border-r border-sidebar-border bg-[hsl(var(--sidebar-bg))] overflow-hidden"
        style={{ width: sidebarWidth }}
      >
        <Sidebar />
      </aside>

      {/* Resize handle 1 */}
      <ResizeHandle
        onDrag={(delta) =>
          setSidebarWidth((w) => Math.min(400, Math.max(160, w + delta)))
        }
      />

      {/* Pane 2: List panel */}
      <div
        className="flex-none border-r border-border overflow-hidden"
        style={{ width: listWidth }}
      >
        <LibraryList />
      </div>

      {/* Resize handle 2 */}
      <ResizeHandle
        onDrag={(delta) =>
          setListWidth((w) => Math.min(600, Math.max(200, w + delta)))
        }
      />

      {/* Pane 3: Detail panel */}
      <main className="flex-1 overflow-hidden flex flex-col min-w-0">
        <Outlet />
      </main>
    </div>
  );
}

function ResizeHandle({ onDrag }: { onDrag: (delta: number) => void }) {
  return (
    <div
      className="no-drag w-1 cursor-col-resize hover:bg-primary/20 active:bg-primary/40 transition-colors flex-none select-none"
      onMouseDown={(e) => {
        e.preventDefault();
        let lastX = e.clientX;

        document.body.style.cursor = "col-resize";
        document.body.style.userSelect = "none";

        const onMove = (ev: MouseEvent) => {
          const delta = ev.clientX - lastX;
          lastX = ev.clientX;
          onDrag(delta);
        };
        const onUp = () => {
          document.body.style.cursor = "";
          document.body.style.userSelect = "";
          document.removeEventListener("mousemove", onMove);
          document.removeEventListener("mouseup", onUp);
        };
        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
      }}
    />
  );
}
