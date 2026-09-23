import React, { useState, useMemo } from 'react';
import { useApp } from '../../context/AppContext';

export const Sidebar: React.FC = () => {
  const {
    stocks,
    currentIndex,
    activeStock,
    selectStock,
    addStock,
    deleteStock,
    sidebarTab,
    setSidebarTab,
    sidebarSearch,
    setSidebarSearch,
    movers,
    moversThreshold,
    setMoversThreshold,
    moversFilter,
    setMoversFilter,
    filteredMovers,
    triggerScan,
    setViewMode,
  } = useApp();

  // Add stock inputs
  const [newSym, setNewSym] = useState('');
  const [newName, setNewName] = useState('');
  const [isAdding, setIsAdding] = useState(false);

  // Filtered universe stocks
  const filteredStocks = useMemo(() => {
    if (!sidebarSearch.trim()) return stocks;
    const q = sidebarSearch.toLowerCase();
    return stocks.filter(
      s => s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
    );
  }, [stocks, sidebarSearch]);

  const handleAddStock = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newSym.trim()) return;
    setIsAdding(true);
    try {
      await addStock(newSym, newName || newSym);
      setNewSym('');
      setNewName('');
    } finally {
      setIsAdding(false);
    }
  };

  return (
    <aside className="sidebar" id="sidebar">
      {/* Sidebar Header */}
      <div className="sidebar-header">
        <div className="sidebar-tabs">
          <button
            className={`tab-btn ${sidebarTab === 'universe' ? 'active' : ''}`}
            onClick={() => setSidebarTab('universe')}
          >
            <span>Universe</span>
            <span className="count-pill" id="count-universe">
              {stocks.length}
            </span>
          </button>
          <button
            className={`tab-btn ${sidebarTab === 'movers' ? 'active' : ''}`}
            onClick={() => setSidebarTab('movers')}
          >
            <span>Top Movers</span>
            <span className="count-pill" id="count-movers">
              {filteredMovers.length}
            </span>
          </button>
        </div>

        {/* Search Bar */}
        <div className="search-wrap">
          <svg
            className="search-icon"
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
          </svg>
          <input
            type="text"
            className="search-input"
            id="search-box"
            placeholder={
              sidebarTab === 'universe'
                ? 'Search stock or symbol... (/) '
                : 'Filter movers...'
            }
            value={sidebarSearch}
            onChange={e => setSidebarSearch(e.target.value)}
          />
        </div>

        {/* Movers Filter Controls */}
        {sidebarTab === 'movers' && (
          <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div className="timeframe-group" style={{ height: '24px' }}>
                {[3.0, 5.0, 7.0].map(th => (
                  <button
                    key={th}
                    className={`tf-btn ${moversThreshold === th ? 'active' : ''}`}
                    style={{ fontSize: '10px', padding: '0 6px' }}
                    onClick={() => {
                      setMoversThreshold(th);
                      triggerScan(false);
                    }}
                  >
                    ±{th}%
                  </button>
                ))}
              </div>
              <button
                className="top-action-btn active"
                style={{ fontSize: '10px', padding: '2px 8px' }}
                onClick={() => triggerScan(true)}
              >
                ⚡ Rescan
              </button>
            </div>

            <div style={{ display: 'flex', gap: '4px' }}>
              {(['all', 'gainers', 'losers'] as const).map(f => (
                <button
                  key={f}
                  className={`filter-pill ${moversFilter === f ? 'active' : ''}`}
                  onClick={() => setMoversFilter(f)}
                  style={{ textTransform: 'capitalize' }}
                >
                  {f === 'gainers' ? 'Gainers 🟢' : f === 'losers' ? 'Losers 🔴' : 'All'}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Stock List Scroll Area */}
      <ul className="stock-list" id="stock-list">
        {sidebarTab === 'universe' ? (
          filteredStocks.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
              No matching stocks found.
            </div>
          ) : (
            filteredStocks.map(stock => {
              const isActive = activeStock?.symbol === stock.symbol;
              const originalIndex = stocks.findIndex(s => s.symbol === stock.symbol);

              return (
                <li
                  key={stock.symbol}
                  className={`stock-row ${isActive ? 'active' : ''}`}
                  onClick={() => {
                    selectStock(originalIndex);
                    setViewMode('single');
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
                    <span className="stock-index-badge">{originalIndex + 1}</span>
                    <div style={{ minWidth: 0 }}>
                      <div className="stock-sym">{stock.symbol}</div>
                      <div className="stock-corp-name">{stock.name}</div>
                    </div>
                  </div>
                  <button
                    className="stock-delete-btn"
                    title={`Remove ${stock.symbol} from Universe`}
                    style={{
                      background: 'none',
                      border: 'none',
                      color: 'var(--text-muted)',
                      cursor: 'pointer',
                      fontSize: '11px',
                      flexShrink: 0,
                    }}
                    onClick={e => {
                      e.stopPropagation();
                      deleteStock(stock.symbol);
                    }}
                  >
                    ✕
                  </button>
                </li>
              );
            })
          )
        ) : filteredMovers.length === 0 ? (
          <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
            No movers found matching ±{moversThreshold}%.
          </div>
        ) : (
          filteredMovers.map(mover => {
            const isActive = activeStock?.symbol === mover.symbol;
            const isPos = mover.change_percent >= 0;

            return (
              <li
                key={mover.symbol}
                className={`stock-row ${isActive ? 'active' : ''}`}
                onClick={() => {
                  selectStock(mover.symbol);
                  setViewMode('single');
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <div className="stock-sym">{mover.symbol}</div>
                  <div className="stock-corp-name">
                    ₹{mover.price.toFixed(2)}
                  </div>
                </div>
                <div
                  style={{
                    fontWeight: 700,
                    fontSize: '12px',
                    color: isPos ? 'var(--bullish)' : 'var(--bearish)',
                    textAlign: 'right',
                  }}
                >
                  {isPos ? '+' : ''}
                  {mover.change_percent.toFixed(2)}%
                </div>
              </li>
            );
          })
        )}
      </ul>

      {/* Add Stock Footer (Universe Tab only) */}
      {sidebarTab === 'universe' && (
        <form className="sidebar-footer" onSubmit={handleAddStock} style={{ display: 'flex', gap: '6px' }}>
          <input
            type="text"
            className="search-input"
            placeholder="TICKER.NS"
            value={newSym}
            onChange={e => setNewSym(e.target.value)}
            style={{ width: '45%' }}
            required
          />
          <input
            type="text"
            className="search-input"
            placeholder="Name (opt)"
            value={newName}
            onChange={e => setNewName(e.target.value)}
            style={{ width: '40%' }}
          />
          <button
            type="submit"
            className="top-action-btn active"
            disabled={isAdding}
            style={{ padding: '4px 8px' }}
          >
            +
          </button>
        </form>
      )}
    </aside>
  );
};
