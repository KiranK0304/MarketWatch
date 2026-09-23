import React from 'react';
import { useApp } from '../../context/AppContext';

export const LeftRail: React.FC = () => {
  const {
    openShortcuts,
    openCommandPalette,
    isDark,
    toggleTheme,
  } = useApp();

  return (
    <aside className="nav-rail">
      <div className="rail-brand" title="MarketWatch Terminal">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2">
          <path d="M3 3v18h18" />
          <path d="m19 9-5 5-4-4-3 3" />
        </svg>
      </div>

      <div className="rail-spacer" />

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

        <button
          className="rail-btn"
          id="btn-rail-theme"
          title="Toggle White / Black Theme (T)"
          onClick={toggleTheme}
        >
          <span id="rail-theme-icon" style={{ fontSize: '15px' }}>{isDark ? '☀️' : '🌙'}</span>
        </button>
      </div>
    </aside>
  );
};
