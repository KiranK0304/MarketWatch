import React from 'react';
import { useApp } from '../../context/AppContext';

export const StatusStrip: React.FC = () => {
  const { marketStatus, marketStatusLoaded, cacheLatency } = useApp();

  return (
    <footer className="status-strip">
      <div className="status-item">
        <div
          className="status-indicator-dot"
          style={{
            background: !marketStatusLoaded
              ? 'var(--text-muted)'
              : marketStatus.is_open
              ? 'var(--bullish)'
              : 'var(--bearish)',
          }}
        />
        <span>
          {!marketStatusLoaded
            ? 'NSE: Checking status...'
            : marketStatus.is_open
            ? 'NSE: Live (09:15 - 15:30 IST)'
            : !marketStatus.is_trading_day
            ? 'NSE: Closed (Holiday / Weekend)'
            : 'NSE: Market Closed'}
        </span>
      </div>

      <div className="status-item">
        <span>
          ⚡ SQLite Cache: <strong>{cacheLatency}</strong>
        </span>
      </div>

      <div className="status-item">
        <span>🔔 09:30 & 15:30 IST Auto-Catchup Active</span>
      </div>

      <div className="status-item" style={{ color: 'var(--text-secondary)' }}>
        <span>
          Press <kbd style={{ background: 'var(--bg-subtle)', padding: '1px 4px', borderRadius: '3px', border: '1px solid var(--border)' }}>Ctrl+K</kbd> for Commands
        </span>
      </div>
    </footer>
  );
};
