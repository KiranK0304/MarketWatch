import React, { useEffect, useState } from 'react';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import type { CacheSyncMeta } from '../../types';

export const FundamentalsTab: React.FC = () => {
  const { activeStock, timeframe, forceRefreshCounter } = useApp();
  const [meta, setMeta] = useState<CacheSyncMeta | null>(null);

  useEffect(() => {
    if (!activeStock) return;
    let isSubscribed = true;
    api.getCacheMeta(activeStock.symbol, timeframe).then(data => {
      if (isSubscribed) setMeta(data);
    });
    return () => {
      isSubscribed = false;
    };
  }, [activeStock, timeframe, forceRefreshCounter]);

  if (!activeStock) {
    return <div style={{ color: 'var(--text-muted)', padding: '20px' }}>Select a stock to view fundamentals.</div>;
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      <div style={{ fontWeight: 700, fontSize: '13px' }}>📊 Technical & Cache Metrics</div>

      <div style={{ background: 'var(--bg-surface)', border: '1px solid var(--border)', borderRadius: '8px', padding: '12px', display: 'flex', flexDirection: 'column', gap: '8px', fontSize: '12px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Symbol</span>
          <span style={{ fontWeight: 600 }}>{activeStock.symbol}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Name</span>
          <span style={{ fontWeight: 600 }}>{activeStock.name}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Timeframe</span>
          <span style={{ fontWeight: 600 }}>{timeframe}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Cached Candles</span>
          <span style={{ fontWeight: 600 }}>{meta ? meta.candle_count : '--'}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Historical Integrity</span>
          <span style={{ fontWeight: 600, color: meta?.has_gap ? 'var(--bearish)' : 'var(--bullish)' }}>
            {meta ? (meta.has_gap ? 'Gap Detected ⚠️' : 'Gap-Free Sequence ✓') : '--'}
          </span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
          <span style={{ color: 'var(--text-muted)' }}>Last Synced</span>
          <span style={{ fontWeight: 600 }}>
            {meta && meta.last_synced_at > 0 ? new Date(meta.last_synced_at * 1000).toLocaleTimeString('en-IN') : '--'}
          </span>
        </div>
      </div>
    </div>
  );
};
