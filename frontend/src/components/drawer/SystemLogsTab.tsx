import React, { useEffect, useState } from 'react';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';

export const SystemLogsTab: React.FC = () => {
  const { activeStock, timeframe, forceRefreshCounter } = useApp();
  const [syncStr, setSyncStr] = useState('Loading...');
  const [barCountStr, setBarCountStr] = useState('0');
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  useEffect(() => {
    if (!activeStock) {
      setSyncStr('Unsynced');
      setBarCountStr('0');
      return;
    }

    let isSubscribed = true;
    api
      .getCacheMeta(activeStock.symbol, timeframe)
      .then(meta => {
        if (!isSubscribed) return;
        if (meta) {
          setBarCountStr(`${meta.candle_count} bars`);
          const d = new Date(meta.last_synced_at * 1000);
          setSyncStr(
            d.toLocaleTimeString('en-IN', {
              hour: '2-digit',
              minute: '2-digit',
              second: '2-digit',
            })
          );
        } else {
          setSyncStr('Unsynced');
          setBarCountStr('0');
        }
      })
      .catch(() => {
        if (isSubscribed) setSyncStr('Error');
      });

    return () => {
      isSubscribed = false;
    };
  }, [activeStock?.symbol, timeframe, forceRefreshCounter]);

  const handleCopy = (txt: string, key: string) => {
    navigator.clipboard.writeText(txt);
    setCopiedKey(key);
    setTimeout(() => {
      setCopiedKey(prev => (prev === key ? null : prev));
    }, 1500);
  };

  return (
    <>
      <div className="drawer-card">
        <div className="drawer-card-title">
          <span>⚙️ 1-Click Operations</span>
          <span style={{ fontSize: '10px', color: 'var(--bullish)' }}>Daemon Active</span>
        </div>

        <div style={{ fontSize: '11px', fontWeight: 600, color: 'var(--text-secondary)' }}>Restart Web Service:</div>
        <div className="cmd-snippet">
          <code>systemctl --user restart marketwatch-web.service</code>
          <button
            className="copy-chip"
            onClick={() => handleCopy('systemctl --user restart marketwatch-web.service', 'restart')}
          >
            {copiedKey === 'restart' ? 'Copied!' : 'Copy'}
          </button>
        </div>

        <div style={{ fontSize: '11px', fontWeight: 600, color: 'var(--text-secondary)', marginTop: '6px' }}>
          Follow Real-Time Logs:
        </div>
        <div className="cmd-snippet">
          <code>journalctl --user -u marketwatch-web.service -f</code>
          <button
            className="copy-chip"
            onClick={() => handleCopy('journalctl --user -u marketwatch-web.service -f', 'logs')}
          >
            {copiedKey === 'logs' ? 'Copied!' : 'Copy'}
          </button>
        </div>

        <div style={{ fontSize: '11px', fontWeight: 600, color: 'var(--text-secondary)', marginTop: '6px' }}>
          Trigger 3% Movers Scan:
        </div>
        <div className="cmd-snippet">
          <code>market scan --threshold 3.0 --notify</code>
          <button
            className="copy-chip"
            onClick={() => handleCopy('market scan --threshold 3.0 --notify', 'scan')}
          >
            {copiedKey === 'scan' ? 'Copied!' : 'Copy'}
          </button>
        </div>
      </div>

      <div className="drawer-card">
        <div className="drawer-card-title">
          <span>🗄️ SQLite Storage Stats</span>
        </div>
        <div style={{ fontSize: '11px', color: 'var(--text-secondary)', lineHeight: 1.6 }}>
          • Active Cache: <strong>{barCountStr}</strong><br />
          • Last Synced: <strong>{syncStr}</strong><br />
          • Database: <code style={{ fontFamily: 'var(--font-mono)', color: 'var(--accent)' }}>~/.config/marketwatch/marketwatch.db</code><br />
          • Concurrency: <strong>WAL (Write-Ahead Logging)</strong>
        </div>
      </div>
    </>
  );
};
