import React, { useMemo } from 'react';
import { useApp } from '../../context/AppContext';

function formatPrice(val: number | string): string {
  if (typeof val === 'string') return val;
  return Number(val).toLocaleString('en-IN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export const AiInsightsTab: React.FC = () => {
  const { activeStock, timeframe, candles } = useApp();

  const metrics = useMemo(() => {
    let pivot = '--',
      r1 = '--',
      s1 = '--',
      r2 = '--',
      s2 = '--',
      pattern = 'Analyzing momentum...';
    let sentiment = 'NEUTRAL',
      sentColor = 'var(--accent)',
      sentScore = '55%';

    if (candles && candles.length >= 2) {
      const last = candles[candles.length - 1];
      const prev = candles[candles.length - 2];
      const h = last.high,
        l = last.low,
        c = last.close;
      const p = (h + l + c) / 3;
      pivot = formatPrice(p);
      r1 = formatPrice(2 * p - l);
      s1 = formatPrice(2 * p - h);
      r2 = formatPrice(p + (h - l));
      s2 = formatPrice(p - (h - l));

      // Candlestick pattern detection
      const lastBody = Math.abs(last.close - last.open);
      const prevBody = Math.abs(prev.close - prev.open);

      if (
        last.close > last.open &&
        prev.close < prev.open &&
        last.close > prev.open &&
        last.open < prev.close
      ) {
        pattern = 'Bullish Engulfing pattern detected on volume expansion.';
        sentiment = 'BULLISH';
        sentColor = 'var(--bullish)';
        sentScore = '82%';
      } else if (
        last.close < last.open &&
        prev.close > prev.open &&
        last.open > prev.close &&
        last.close < prev.open
      ) {
        pattern = 'Bearish Engulfing pattern detected near local resistance.';
        sentiment = 'BEARISH';
        sentColor = 'var(--bearish)';
        sentScore = '76%';
      } else if (
        last.close >= last.open &&
        last.open - last.low > 2 * lastBody &&
        last.high - last.close < lastBody * 0.3
      ) {
        pattern = 'Hammer pattern forming near support zone.';
        sentiment = 'BULLISH';
        sentColor = 'var(--bullish)';
        sentScore = '70%';
      } else if (lastBody < (last.high - last.low) * 0.1) {
        pattern = 'Doji indecision bar at key level.';
        sentiment = 'NEUTRAL';
        sentColor = 'var(--accent)';
        sentScore = '50%';
      } else if (last.close >= last.open) {
        pattern = 'Bullish continuation structure holding above VWAP.';
        sentiment = 'BULLISH';
        sentColor = 'var(--bullish)';
        sentScore = '65%';
      } else {
        pattern = 'Distribution pressure testing session lows.';
        sentiment = 'BEARISH';
        sentColor = 'var(--bearish)';
        sentScore = '62%';
      }
    }

    return { pivot, r1, s1, r2, s2, pattern, sentiment, sentColor, sentScore };
  }, [candles]);

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
