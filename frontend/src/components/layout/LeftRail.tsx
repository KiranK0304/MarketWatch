import React from 'react';
import { useApp } from '../../context/AppContext';

export const LeftRail: React.FC = () => {
  const {
    sidebarTab,
    setSidebarTab,
    viewMode,
    setViewMode,
    openShortcuts,
    openCommandPalette,
    stocks,
    filteredMovers,
  } = useApp();

  return (
    <aside className="nav-rail">
      <div className="rail-brand" title="MarketWatch Terminal">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2">
          <path d="M3 3v18h18" />
          <path d="m19 9-5 5-4-4-3 3" />
        </svg>
      </div>

      <div className="rail-nav">
        <button
          className={`rail-btn ${sidebarTab === 'universe' && viewMode === 'single' ? 'active' : ''}`}
          title="Watchlist & All Stocks"
          onClick={() => {
            setSidebarTab('universe');
            setViewMode('single');
          }}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M3 3v18h18" />
            <path d="M18 17V9" />
            <path d="M13 17V5" />
            <path d="M8 17v-3" />
          </svg>
          <span className="rail-badge">{stocks.length}</span>
        </button>

        <button
          className={`rail-btn ${sidebarTab === 'movers' && viewMode === 'single' ? 'active' : ''}`}
          title="Top Movers & Breakouts"
          onClick={() => {
            setSidebarTab('movers');
            setViewMode('single');
          }}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
          </svg>
          <span className="rail-badge">{filteredMovers.length}</span>
        </button>

        <button
          className={`rail-btn ${viewMode === 'grid' ? 'active' : ''}`}
          title="Multi-Chart Grid View (G)"
          onClick={() => setViewMode(viewMode === 'grid' ? 'single' : 'grid')}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect width="7" height="7" x="3" y="3" rx="1" />
            <rect width="7" height="7" x="14" y="3" rx="1" />
            <rect width="7" height="7" x="14" y="14" rx="1" />
            <rect width="7" height="7" x="3" y="14" rx="1" />
          </svg>
        </button>
      </div>

      <div className="rail-footer">
        <button className="rail-btn" title="Command Palette (Ctrl+K)" onClick={openCommandPalette}>
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
          </svg>
        </button>

        <button className="rail-btn" title="Shortcuts Guide (?)" onClick={openShortcuts}>
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="10" />
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
            <path d="M12 17h.01" />
          </svg>
        </button>
      </div>
    </aside>
  );
};
