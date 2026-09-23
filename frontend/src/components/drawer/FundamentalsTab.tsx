import React from 'react';
import { useApp } from '../../context/AppContext';

export const FundamentalsTab: React.FC = () => {
  const { activeStock, timeframe, candles } = useApp();

  if (!activeStock) {
    return <div style={{ color: 'var(--text-muted)', padding: '20px' }}>Select a stock to view fundamentals.</div>;
  }

  return (
    <>
      <div className="drawer-card">
        <div className="drawer-card-title">
          <span>📊 Active Stock Information</span>
          <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>{activeStock.symbol}</span>
        </div>
        <div className="metric-grid">
          <div className="metric-box">
            <div className="metric-lbl">Symbol</div>
            <div className="metric-val" style={{ fontSize: '11px' }}>{activeStock.symbol}</div>
          </div>
          <div className="metric-box">
            <div className="metric-lbl">Exchange</div>
            <div className="metric-val">NSE (India)</div>
          </div>
          <div className="metric-box">
            <div className="metric-lbl">Loaded Bars</div>
            <div className="metric-val">{candles ? candles.length : 0} bars</div>
          </div>
          <div className="metric-box">
            <div className="metric-lbl">Timeframe</div>
            <div className="metric-val">{timeframe}</div>
          </div>
        </div>
      </div>

      <div className="drawer-card">
        <div className="drawer-card-title">
          <span>🏢 Corporate Profile</span>
        </div>
        <div style={{ fontSize: '11px', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
          <strong>{activeStock.name}</strong> is tracked in your custom stock universe. Historical data is cached in SQLite for microsecond load times.
        </div>
      </div>
    </>
  );
};
