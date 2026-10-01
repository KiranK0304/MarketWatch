import React, { useCallback, useState } from 'react';
import { useApp } from '../../context/AppContext';
import { AiInsightsTab } from './AiInsightsTab';
import { JournalTab } from './JournalTab';
import { FundamentalsTab } from './FundamentalsTab';
import { SystemLogsTab } from './SystemLogsTab';
import type { DrawerTab } from '../../types';

export const IntelDrawer: React.FC = () => {
  const { isDrawerOpen, toggleDrawer, drawerTab, setDrawerTab, drawerWidth, setDrawerWidth } = useApp();
  const [isDragging, setIsDragging] = useState(false);
  const drawerRef = React.useRef<HTMLElement>(null);

  const tabs: { key: DrawerTab; label: string; icon: string }[] = [
    { key: 'ai', label: 'AI', icon: '🧠' },
    { key: 'journal', label: 'Journal', icon: '📝' },
    { key: 'fundamentals', label: 'Fundamentals', icon: '📊' },
    { key: 'system', label: 'System', icon: '⚙️' },
  ];

  // Direct cursor tracking for drawer width (expands to the left)
  const startResizing = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setIsDragging(true);

      const onMouseMove = (moveEvent: MouseEvent) => {
        // Measure viewport parent width so the drawer never overflows the viewport
        const viewportArea = drawerRef.current?.parentElement;
        const availableWidth = viewportArea ? viewportArea.clientWidth : (window.innerWidth - 300);
        // Leave at least 250px for the chart/grid on the left
        const maxDrawerWidth = Math.max(260, availableWidth - 250);
        const newWidth = Math.max(260, Math.min(maxDrawerWidth, window.innerWidth - moveEvent.clientX));
        setDrawerWidth(newWidth);
      };

      const onMouseUp = () => {
        setIsDragging(false);
        window.removeEventListener('mousemove', onMouseMove);
        window.removeEventListener('mouseup', onMouseUp);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      };

      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
      window.addEventListener('mousemove', onMouseMove);
      window.addEventListener('mouseup', onMouseUp);
    },
    [setDrawerWidth]
  );

  return (
    <>
      {/* Full-screen drag overlay to prevent canvas / text from stealing mouse events */}
      {isDragging && <div className="resizer-drag-overlay" />}

      <aside
        ref={drawerRef}
        className={`intel-drawer ${isDrawerOpen ? '' : 'collapsed'}`}
        id="intel-drawer"
        style={{
          width: isDrawerOpen ? `${drawerWidth}px` : 0,
          minWidth: isDrawerOpen ? undefined : 0,
        }}
      >
        {/* Resizer Handle on the left border */}
        {isDrawerOpen && (
          <div
            className={`drawer-resize-handle ${isDragging ? 'active' : ''}`}
            onMouseDown={startResizing}
            title="Drag to resize intel panel"
          />
        )}

        <div className="drawer-header">
          <div className="drawer-nav-tabs">
            {tabs.map(t => (
              <button
                key={t.key}
                className={`drawer-tab ${drawerTab === t.key ? 'active' : ''}`}
                title={t.key === 'ai' ? 'AI Insights' : t.key === 'system' ? 'System & Logs' : t.label}
                onClick={() => setDrawerTab(t.key)}
              >
                <span>{t.icon}</span>
                <span>{t.label}</span>
              </button>
            ))}
          </div>
          <button
            type="button"
            className="top-action-btn"
            style={{
              padding: '3px 7px',
              fontSize: '12px',
              flexShrink: 0,
              color: 'var(--text-muted)',
              cursor: 'pointer',
            }}
            id="btn-close-drawer"
            title="Close Drawer (I)"
            onClick={toggleDrawer}
          >
            ✕
          </button>
        </div>

        <div className="drawer-body" id="drawer-content">
          {drawerTab === 'ai' && <AiInsightsTab />}
          {drawerTab === 'journal' && <JournalTab />}
          {drawerTab === 'fundamentals' && <FundamentalsTab />}
          {drawerTab === 'system' && <SystemLogsTab />}
        </div>
      </aside>
    </>
  );
};
