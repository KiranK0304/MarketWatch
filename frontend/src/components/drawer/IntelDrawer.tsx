import React from 'react';
import { useApp } from '../../context/AppContext';
import { AiInsightsTab } from './AiInsightsTab';
import { JournalTab } from './JournalTab';
import { FundamentalsTab } from './FundamentalsTab';
import { SystemLogsTab } from './SystemLogsTab';
import type { DrawerTab } from '../../types';

export const IntelDrawer: React.FC = () => {
  const { isDrawerOpen, toggleDrawer, drawerTab, setDrawerTab } = useApp();

  const tabs: { key: DrawerTab; label: string }[] = [
    { key: 'ai', label: '🧠 AI Insights' },
    { key: 'journal', label: '📝 Journal' },
    { key: 'fundamentals', label: '📊 Fundamentals' },
    { key: 'system', label: '⚙️ System & Logs' },
  ];

  return (
    <aside className={`intel-drawer ${isDrawerOpen ? '' : 'collapsed'}`} id="intel-drawer">
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
