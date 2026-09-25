import React, { useMemo } from 'react';
import { useApp } from '../../context/AppContext';
import { calculateInsights } from '../../domain/insights';

export const AiInsightsTab: React.FC = () => {
  const { activeStock, timeframe, candles } = useApp();

  const metrics = useMemo(() => calculateInsights(candles), [candles]);

  if (!activeStock) {
    return <div style={{ color: 'var(--text-muted)', padding: '20px' }}>Select a stock to view AI analysis.</div>;
  }

  return (
    <>
      <div className="ai-insight-box">
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span style={{ fontWeight: 700, fontSize: '12px', color: 'var(--purple)' }}>🧠 Technical & Pattern Synthesis</span>
          <span
            style={{
              fontSize: '10px',
              background: metrics.sentColor,
              color: 'white',
              padding: '2px 6px',
              borderRadius: '4px',
              fontWeight: 700,
            }}
          >
            {metrics.sentiment} ({metrics.sentScore})
          </span>
        </div>
        <div style={{ fontSize: '11px', color: 'var(--text-secondary)', lineHeight: 1.5, marginTop: '4px' }}>
          {activeStock.symbol} ({timeframe}): {metrics.pattern}
        </div>
      </div>

      <div className="drawer-card">
        <div className="drawer-card-title">
          <span>🎯 Algorithmic Floor Pivots</span>
          <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>{timeframe} Bar</span>
        </div>
        <div className="metric-grid">
          <div className="metric-box">
            <div className="metric-lbl">Resistance 2 (R2)</div>
            <div className="metric-val" style={{ color: 'var(--bearish)' }}>₹{metrics.r2}</div>
          </div>
          <div className="metric-box">
            <div className="metric-lbl">Resistance 1 (R1)</div>
            <div className="metric-val" style={{ color: 'var(--bearish)' }}>₹{metrics.r1}</div>
          </div>
          <div className="metric-box">
            <div className="metric-lbl">Central Pivot (P)</div>
            <div className="metric-val" style={{ color: 'var(--accent)' }}>₹{metrics.pivot}</div>
          </div>
          <div className="metric-box">
            <div className="metric-lbl">Support 1 (S1)</div>
            <div className="metric-val" style={{ color: 'var(--bullish)' }}>₹{metrics.s1}</div>
          </div>
          <div className="metric-box" style={{ gridColumn: '1 / -1' }}>
            <div className="metric-lbl">Support 2 (S2)</div>
            <div className="metric-val" style={{ color: 'var(--bullish)' }}>₹{metrics.s2}</div>
          </div>
        </div>
      </div>

      <div className="drawer-card">
        <div className="drawer-card-title">
          <span>⚡ AI Research Harness Roadmap</span>
        </div>
        <div style={{ fontSize: '11px', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
          • Real-time RSS & Corporate Filing News Ingestion<br />
          • LLM Multi-Timeframe Confluence Synthesis<br />
          • Risk/Reward Ratio & Stop Loss Engine
        </div>
      </div>
    </>
  );
};
