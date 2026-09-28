import React, { useCallback } from 'react';
import { useApp } from '../../context/AppContext';
import { AiInsightsTab } from './AiInsightsTab';
import { JournalTab } from './JournalTab';
import { FundamentalsTab } from './FundamentalsTab';
import { SystemLogsTab } from './SystemLogsTab';
import type { DrawerTab } from '../../types';

export const IntelDrawer: React.FC = () => {
  const { isDrawerOpen, toggleDrawer, drawerTab, setDrawerTab, drawerWidth, setDrawerWidth } = useApp();

  const tabs: { key: DrawerTab; label: string }[] = [
    { key: 'ai', label: '🧠 AI Insights' },
    { key: 'journal', label: '📝 Journal' },
    { key: 'fundamentals', label: '📊 Fundamentals' },
    { key: 'system', label: '⚙️ System & Logs' },
  ];

  // Drag resizer for drawer width (expands to the left)
  const startResizing = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startWidth = drawerWidth;

      const onMouseMove = (moveEvent: MouseEvent) => {
        // As mouse moves left (smaller clientX), drawer width increases
        const delta = startX - moveEvent.clientX;
        const newWidth = Math.max(260, Math.min(750, startWidth + delta));
        setDrawerWidth(newWidth);
      };

      const onMouseUp = () => {
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
    [drawerWidth, setDrawerWidth]
  );

  return (
    <aside
      className={`intel-drawer ${isDrawerOpen ? '' : 'collapsed'}`}
      id="intel-drawer"
      style={{
        width: isDrawerOpen ? `${drawerWidth}px` : 0,
        position: 'relative',
      }}
    >
      {/* Resizer Handle on the left border */}
      {isDrawerOpen && (
        <div
          className="drawer-resize-handle"
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
              onClick={() => setDrawerTab(t.key)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <button
          style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', fontSize: '14px' }}
          id="btn-close-drawer"
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
  );
};
