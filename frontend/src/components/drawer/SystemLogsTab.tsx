import React, { useEffect, useState } from 'react';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import type { ScanState } from '../../types';

export const SystemLogsTab: React.FC = () => {
  const { triggerScan, triggerForceRefresh, showToast, marketStatus, cacheLatency, chartCacheHeader } = useApp();
  const [scanState, setScanState] = useState<ScanState | null>(null);

  useEffect(() => {
    api.getScanState().then(setScanState);
  }, []);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      <div style={{ fontWeight: 700, fontSize: '13px' }}>⚙️ System Engine & Scheduler</div>

      <div style={{ background: 'var(--bg-surface)', border: '1px solid var(--border)', borderRadius: '8px', padding: '12px', display: 'flex', flexDirection: 'column', gap: '8px', fontSize: '12px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Market Session</span>
          <span style={{ fontWeight: 600, color: marketStatus.is_open ? 'var(--bullish)' : 'var(--bearish)' }}>
            {marketStatus.is_open ? 'Live Trading (09:15 - 15:30 IST)' : 'Market Closed'}
          </span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Trading Day</span>
          <span style={{ fontWeight: 600 }}>{marketStatus.is_trading_day ? 'Official Trading Day ✓' : 'Weekend / Holiday'}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Cache Response</span>
          <span style={{ fontWeight: 600 }}>{chartCacheHeader} ({cacheLatency})</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: '4px' }}>
          <span style={{ color: 'var(--text-muted)' }}>Morning Scan (09:30 IST)</span>
          <span style={{ fontWeight: 600 }}>{scanState?.last_morning_scan_date || 'Pending'}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
          <span style={{ color: 'var(--text-muted)' }}>Evening Scan (15:30 IST)</span>
          <span style={{ fontWeight: 600 }}>{scanState?.last_evening_scan_date || 'Pending'}</span>
        </div>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '6px' }}>
        <button
          className="top-action-btn active"
          style={{ justifyContent: 'center', padding: '8px' }}
          onClick={() => triggerScan(true)}
        >
          ⚡ Run Market Movers Scanner
        </button>
        <button
          className="top-action-btn"
          style={{ justifyContent: 'center', padding: '8px' }}
          onClick={() => {
            triggerForceRefresh();
            showToast('Force refreshed active stock cache');
          }}
        >
          🔄 Force Sync Current Stock Cache
        </button>
      </div>
    </div>
  );
};
