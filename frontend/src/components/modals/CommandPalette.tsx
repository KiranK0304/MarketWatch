import React, { useState, useMemo, useEffect, useRef } from 'react';
import { useApp } from '../../context/AppContext';

export const CommandPalette: React.FC = () => {
  const {
    isCommandPaletteOpen,
    closeCommandPalette,
    setTimeframe,
    setViewMode,
    openNoteModal,
    triggerScan,
    triggerForceRefresh,
    toggleDrawer,
    openShortcuts,
    stocks,
    selectStock,
  } = useApp();

  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isCommandPaletteOpen) {
      setQuery('');
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isCommandPaletteOpen]);

  const commands = useMemo(() => {
    const list: { id: string; label: string; group: string; action: () => void }[] = [
      { id: 'note', label: 'Log Analysis Note (N)', group: 'Journal', action: () => openNoteModal() },
      { id: 'single', label: 'View: Single Focus Chart', group: 'Navigation', action: () => setViewMode('single') },
      { id: 'grid', label: 'View: Multi-Chart Grid (G)', group: 'Navigation', action: () => setViewMode('grid') },
      { id: 'tf-15m', label: 'Timeframe: 15 Minutes (2)', group: 'Timeframe', action: () => setTimeframe('15m') },
      { id: 'tf-1h', label: 'Timeframe: 1 Hour (4)', group: 'Timeframe', action: () => setTimeframe('1h') },
      { id: 'tf-1d', label: 'Timeframe: 1 Day (5)', group: 'Timeframe', action: () => setTimeframe('1d') },
      { id: 'scan', label: 'Run Market Movers Scanner', group: 'Actions', action: () => triggerScan(true) },
      { id: 'sync', label: 'Force Sync Yahoo Finance Cache (F)', group: 'Actions', action: () => triggerForceRefresh() },
      { id: 'drawer', label: 'Toggle Intel & AI Panel (I)', group: 'Navigation', action: () => toggleDrawer() },
      { id: 'shortcuts', label: 'Open Keyboard Shortcuts Guide (?)', group: 'Help', action: () => openShortcuts() },
    ];

    // Add stocks to palette for quick navigation
    if (query.trim()) {
      const q = query.toLowerCase();
      const matchedStocks = stocks
        .filter(s => s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q))
        .slice(0, 8)
        .map(s => ({
          id: `stock-${s.symbol}`,
          label: `Jump to ${s.symbol} (${s.name})`,
          group: 'Stocks',
          action: () => {
            selectStock(s.symbol);
            setViewMode('single');
          },
        }));
      return [...matchedStocks, ...list.filter(c => c.label.toLowerCase().includes(q))];
    }

    return list;
  }, [stocks, query, openNoteModal, setViewMode, setTimeframe, triggerScan, triggerForceRefresh, toggleDrawer, openShortcuts, selectStock]);

  if (!isCommandPaletteOpen) return null;

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (commands.length === 0) return;
      setSelectedIndex(prev => (prev + 1) % commands.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (commands.length === 0) return;
      setSelectedIndex(prev => (prev - 1 + commands.length) % commands.length);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (commands[selectedIndex]) {
        commands[selectedIndex].action();
        closeCommandPalette();
      }
    } else if (e.key === 'Escape') {
      closeCommandPalette();
    }
  };

  return (
    <div className="palette-backdrop open" onClick={closeCommandPalette}>
      <div className="palette-box" onClick={e => e.stopPropagation()}>
        <div className="palette-input-box">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            className="palette-input"
            placeholder="Type a command or stock symbol... (Esc to exit)"
            value={query}
            onChange={e => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleKeyDown}
          />
        </div>

        <div className="palette-list">
          {commands.length === 0 ? (
            <div style={{ padding: '16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
              No matching commands or stocks.
            </div>
          ) : (
            commands.map((cmd, idx) => (
              <div
                key={cmd.id}
                className={`palette-item ${idx === selectedIndex ? 'selected' : ''}`}
                onClick={() => {
                  cmd.action();
                  closeCommandPalette();
                }}
                onMouseEnter={() => setSelectedIndex(idx)}
              >
                <span>{cmd.label}</span>
                <span className="palette-item-tag">{cmd.group}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
};
