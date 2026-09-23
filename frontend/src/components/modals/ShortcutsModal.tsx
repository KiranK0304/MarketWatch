import React from 'react';
import { useApp } from '../../context/AppContext';

export const ShortcutsModal: React.FC = () => {
  const { isShortcutsOpen, closeShortcuts } = useApp();

  if (!isShortcutsOpen) return null;

  const shortcuts = [
    { key: '1 - 6', desc: 'Select Timeframe (5m, 15m, 30m, 1h, 1d, 1w)' },
    { key: '↑ / ↓ or J / K', desc: 'Navigate Stocks Universe' },
    { key: 'Space / F', desc: 'Force Sync fresh data from Yahoo Finance' },
    { key: 'N', desc: 'Log Trading Analysis Note for active stock' },
    { key: 'I', desc: 'Toggle Intel & AI Side Drawer' },
    { key: 'G', desc: 'Toggle Single Focus Chart / Multi-Chart Grid' },
    { key: '/', desc: 'Focus Watchlist Search Filter' },
    { key: 'T', desc: 'Toggle White / Black Theme' },
    { key: '+ / -', desc: 'Zoom In / Out Candlestick Spacing' },
    { key: 'Ctrl + K', desc: 'Open Command Palette' },
    { key: '?', desc: 'Open this Keyboard Shortcuts Guide' },
    { key: 'Esc', desc: 'Close any open modal dialog' },
  ];

  return (
    <div className="modal-backdrop open" onClick={closeShortcuts}>
      <div className="modal-card" style={{ width: '480px', maxWidth: '92vw' }} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid var(--border)', paddingBottom: '10px' }}>
          <div style={{ fontWeight: 700, fontSize: '15px', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span>⌨️</span>
            <span>Keyboard Shortcuts Guide</span>
          </div>
          <button
            style={{ background: 'none', border: 'none', fontSize: '22px', cursor: 'pointer', color: 'var(--text-muted)' }}
            onClick={closeShortcuts}
          >
            &times;
          </button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', margin: '10px 0' }}>
          {shortcuts.map(s => (
            <div key={s.key} className="shortcut-row">
              <span style={{ color: 'var(--text-secondary)' }}>{s.desc}</span>
              <kbd>{s.key}</kbd>
            </div>
          ))}
        </div>

        <div style={{ display: 'flex', justifyContent: 'flex-end', borderTop: '1px solid var(--border)', paddingTop: '10px' }}>
          <button className="top-action-btn" onClick={closeShortcuts}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
