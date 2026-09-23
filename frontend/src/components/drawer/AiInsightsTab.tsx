import React, { useState } from 'react';
import { useApp } from '../../context/AppContext';

export const AiInsightsTab: React.FC = () => {
  const { activeStock, timeframe } = useApp();
  const [copied, setCopied] = useState(false);

  if (!activeStock) {
    return <div style={{ color: 'var(--text-muted)', padding: '20px' }}>Select a stock to view AI analysis.</div>;
  }

  const analysisText = `Technical Setup Analysis for ${activeStock.symbol} (${timeframe}):
- Primary Trend: Consolidation with bullish continuation bias above 20 EMA.
- Momentum Indicator: RSI hovering around 58 with expanding volume on up-swings.
- Key Breakout Level: Immediate resistance at 52-week swing high.
- Risk/Reward: Favorable entry upon confirmed breakout with tight stop below VWAP.`;

  const handleCopy = () => {
    navigator.clipboard.writeText(analysisText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ fontWeight: 700, fontSize: '13px' }}>🧠 Momentum Intelligence</div>
        <button
          className="top-action-btn"
          style={{ fontSize: '10px', padding: '2px 8px' }}
          onClick={handleCopy}
        >
          {copied ? 'Copied! ✓' : 'Copy Thesis 📋'}
        </button>
      </div>

      <div style={{ background: 'var(--bg-surface)', border: '1px solid var(--border)', borderRadius: '8px', padding: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
          <span style={{ fontSize: '11px', fontWeight: 600, color: 'var(--text-secondary)' }}>Trend Bias:</span>
          <span style={{ fontSize: '11px', fontWeight: 700, color: 'var(--bullish)', background: 'var(--bullish-subtle)', padding: '2px 6px', borderRadius: '4px' }}>
            Bullish Continuation
          </span>
        </div>
        <div style={{ fontSize: '12px', color: 'var(--text-secondary)', lineHeight: 1.5, whiteSpace: 'pre-line' }}>
          {analysisText}
        </div>
      </div>
    </div>
  );
};
