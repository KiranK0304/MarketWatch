import React from 'react';
import { useApp } from '../../context/AppContext';
import type { Timeframe } from '../../types';

interface TopBarProps {
  heroPrice: string;
  heroChange: { text: string; isPositive: boolean };
}

export const TopBar: React.FC<TopBarProps> = ({ heroPrice, heroChange }) => {
  const {
    activeStock,
    timeframe,
    setTimeframe,
    viewMode,
    setViewMode,
    gridScope,
    setGridScope,
    gridSearch,
    setGridSearch,
    gridPage,
    setGridPage,
    gridPageSize,
    stocks,
    filteredMovers,
    isDrawerOpen,
    toggleDrawer,
    openNoteModal,
    openShortcuts,
    triggerForceRefresh,
    zoom,
    setZoom,
    zoomIn,
    zoomOut,
  } = useApp();

  const timeframes: Timeframe[] = ['5m', '15m', '30m', '1h', '1d', '1w'];

  // Grid pagination calculations (post-search, mirroring MultiChartGrid's
  // filter — counting pre-search totals showed e.g. "1 / 7" over an empty grid).
  const gridItems =
    gridScope === 'universe'
      ? stocks
      : filteredMovers.map(m => ({ symbol: m.symbol, name: m.name }));
  const gridQuery = gridSearch.trim().toLowerCase();
  const totalGridItems = gridQuery
    ? gridItems.filter(
        s =>
          s.symbol.toLowerCase().includes(gridQuery) ||
          s.name.toLowerCase().includes(gridQuery)
      ).length
    : gridItems.length;
  const totalPages = Math.max(1, Math.ceil(totalGridItems / gridPageSize));

  return (
    <header className="top-control-bar">
      {/* 1. Hero Symbol & Price Info */}
      <div className="hero-stock-info">
        <h1 className="hero-symbol" id="hero-sym">
          {activeStock?.symbol || '--'}
        </h1>
        <div className="hero-price" id="hero-price">
          {heroPrice || '₹--'}
        </div>
        <div
          className={`hero-change-badge ${heroChange.isPositive ? 'up' : 'down'}`}
          id="hero-chg"
        >
          {heroChange.text || '--'}
        </div>
        <div className="hero-meta-divider" />
        <div className="hero-meta-field">
          <span className="hero-meta-label">Exchange</span>
          <span className="hero-meta-val">NSE (India)</span>
        </div>
      </div>

      {/* 2. Center Tools: Timeframe & View Mode Switchers */}
      <div className="header-center-tools">
        {/* Timeframe Group */}
        <div className="timeframe-group" id="tf-group">
          {timeframes.map(tf => (
            <button
              key={tf}
              className={`tf-btn ${timeframe === tf ? 'active' : ''}`}
              onClick={() => setTimeframe(tf)}
            >
              {tf}
            </button>
          ))}
        </div>

        {/* View Mode Switcher */}
        <div className="view-mode-group">
          <button
            className={`view-mode-btn ${viewMode === 'single' ? 'active' : ''}`}
            id="btn-view-single"
            onClick={() => setViewMode('single')}
            title="Single Focus Chart"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <rect width="18" height="18" x="3" y="3" rx="2" />
            </svg>
            <span>Single</span>
          </button>
          <button
            className={`view-mode-btn ${viewMode === 'grid' ? 'active' : ''}`}
            id="btn-view-grid"
            onClick={() => setViewMode('grid')}
            title="Multi-Chart Grid View (G)"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <rect width="7" height="7" x="3" y="3" rx="1" />
              <rect width="7" height="7" x="14" y="3" rx="1" />
              <rect width="7" height="7" x="14" y="14" rx="1" />
              <rect width="7" height="7" x="3" y="14" rx="1" />
            </svg>
            <span>Grid</span>
          </button>
        </div>

        {/* Zoom Dial / Bar Spacing */}
        {viewMode === 'single' && (
          <div className="zoom-dial-wrapper" title="Candlestick zoom / bar spacing (+ / -)">
            <button
              type="button"
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-secondary)', fontSize: '11px', padding: '0 2px' }}
              title="Zoom Out (-)"
              onClick={zoomOut}
            >
              ➖
            </button>
            <input
              type="range"
              id="zoom-range"
              min="1"
              max="10"
              step="0.5"
              value={zoom}
              onChange={e => setZoom(parseFloat(e.target.value))}
            />
            <span id="zoom-val" style={{ minWidth: '24px', textAlign: 'center' }}>{zoom}px</span>
            <button
              type="button"
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-secondary)', fontSize: '11px', padding: '0 2px' }}
              title="Zoom In (+)"
              onClick={zoomIn}
            >
              ➕
            </button>
          </div>
        )}

        {/* Grid-Specific Controls (Scope, Search, Pagination) */}
        {viewMode === 'grid' && (
          <div className="grid-controls-bar" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <div className="view-mode-group" style={{ height: '26px' }}>
              <button
                className={`view-mode-btn ${gridScope === 'universe' ? 'active' : ''}`}
                style={{ fontSize: '11px', padding: '0 8px' }}
                onClick={() => {
                  setGridScope('universe');
                  setGridPage(1);
                }}
              >
                All ({stocks.length})
              </button>
              <button
                className={`view-mode-btn ${gridScope === 'movers' ? 'active' : ''}`}
                style={{ fontSize: '11px', padding: '0 8px' }}
                onClick={() => {
                  setGridScope('movers');
                  setGridPage(1);
                }}
              >
                Movers ({filteredMovers.length})
              </button>
            </div>

            <div className="search-wrap" style={{ width: '130px', height: '26px' }}>
              <input
                type="text"
                className="search-input"
                placeholder="Filter grid..."
                value={gridSearch}
                onChange={e => {
                  setGridSearch(e.target.value);
                  setGridPage(1);
                }}
                style={{ fontSize: '11px', padding: '0 8px' }}
              />
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '11px', color: 'var(--text-muted)' }}>
              <button
                className="top-action-btn"
                style={{ padding: '2px 6px', fontSize: '10px' }}
                disabled={gridPage <= 1}
                onClick={() => setGridPage(p => Math.max(1, p - 1))}
              >
                ◀
              </button>
              <span>
                {gridPage} / {totalPages}
              </span>
              <button
                className="top-action-btn"
                style={{ padding: '2px 6px', fontSize: '10px' }}
                disabled={gridPage >= totalPages}
                onClick={() => setGridPage(p => Math.min(totalPages, p + 1))}
              >
                ▶
              </button>
            </div>
          </div>
        )}
      </div>

      {/* 3. Right Action Tools */}
      <div className="header-right-tools">
        {/* + Note Button */}
        <button
          className="top-action-btn active"
          id="btn-add-note"
          title="Log Analysis Note for current setup (N)"
          onClick={() => openNoteModal()}
        >
          <span>📝</span>
          <span>+ Note</span>
        </button>

        {/* Force Sync Button */}
        <button
          className="top-action-btn"
          id="btn-force-refresh"
          title="Force Sync fresh data from Yahoo (F / Space)"
          onClick={triggerForceRefresh}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
            <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
            <path d="M16 21h5v-5" />
          </svg>
          <span>Sync</span>
        </button>

        {/* Shortcuts Trigger */}
        <button className="top-action-btn" id="btn-open-shortcuts" title="Shortcuts Guide (?)" onClick={openShortcuts}>
          <span>⌨️</span>
          <span>Shortcuts</span>
        </button>

        {/* Intel & AI Drawer Toggle */}
        <button
          className={`top-action-btn ${isDrawerOpen ? 'active' : ''}`}
          id="btn-toggle-intel"
          title="Toggle Intel & AI Panel (I)"
          onClick={toggleDrawer}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
            <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
          </svg>
          <span>Intel & AI</span>
        </button>
      </div>
    </header>
  );
};
