import React, { useState, useMemo, useCallback } from 'react';
import { useApp } from '../../context/AppContext';

export const Sidebar: React.FC = React.memo(() => {
  const {
    stocks,
    activeStock,
    selectStock,
    addStock,
    deleteStock,
    sidebarTab,
    setSidebarTab,
    sidebarSearch,
    setSidebarSearch,
    moversThreshold,
    setMoversThreshold,
    moversFilter,
    setMoversFilter,
    filteredMovers,
    triggerScan,
    setViewMode,
    setGridPage,
    // Watchlists
    watchlists,
    activeWatchlistId,
    setActiveWatchlistId,
    createWatchlist,
    deleteWatchlist,
    renameWatchlist,
    addToWatchlist,
    removeFromWatchlist,
    // Journal Stocks
    journalStocks,
    // Dimensions
    sidebarWidth,
    setSidebarWidth,
  } = useApp();

  // Add stock inputs (Universe)
  const [newSym, setNewSym] = useState('');
  const [newName, setNewName] = useState('');
  const [isAdding, setIsAdding] = useState(false);

  // Watchlist management state
  const [isCreatingWatchlist, setIsCreatingWatchlist] = useState(false);
  const [newWatchlistName, setNewWatchlistName] = useState('');
  const [newWlStockSym, setNewWlStockSym] = useState('');
  const [isRenaming, setIsRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState('');

  // Map symbol to its index in the original universe for O(1) lookups
  const stockIndexMap = useMemo(() => {
    const map = new Map<string, number>();
    for (let i = 0; i < stocks.length; i++) {
      map.set(stocks[i].symbol, i);
    }
    return map;
  }, [stocks]);

  // Current selected watchlist
  const currentWatchlist = useMemo(() => {
    return watchlists.find(w => w.id === activeWatchlistId) || null;
  }, [watchlists, activeWatchlistId]);

  // Stocks in active watchlist
  const watchlistStocks = useMemo(() => {
    if (!currentWatchlist) return [];
    return currentWatchlist.symbols.map(sym => {
      const found = stocks.find(s => s.symbol.toUpperCase() === sym.toUpperCase());
      return found || { symbol: sym, name: sym };
    });
  }, [currentWatchlist, stocks]);

  // Filtered universe stocks
  const filteredStocks = useMemo(() => {
    if (!sidebarSearch.trim()) return stocks;
    const q = sidebarSearch.toLowerCase();
    return stocks.filter(
      s => s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
    );
  }, [stocks, sidebarSearch]);

  // Filtered movers by search query
  const searchedMovers = useMemo(() => {
    if (!sidebarSearch.trim()) return filteredMovers;
    const q = sidebarSearch.toLowerCase();
    return filteredMovers.filter(
      m => m.symbol.toLowerCase().includes(q) || (m.name && m.name.toLowerCase().includes(q))
    );
  }, [filteredMovers, sidebarSearch]);

  // Filtered watchlist stocks by search query
  const searchedWatchlistStocks = useMemo(() => {
    if (!sidebarSearch.trim()) return watchlistStocks;
    const q = sidebarSearch.toLowerCase();
    return watchlistStocks.filter(
      s => s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
    );
  }, [watchlistStocks, sidebarSearch]);

  // Filtered journal stocks by search query
  const searchedJournalStocks = useMemo(() => {
    if (!sidebarSearch.trim()) return journalStocks;
    const q = sidebarSearch.toLowerCase();
    return journalStocks.filter(
      s => s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
    );
  }, [journalStocks, sidebarSearch]);

  const handleAddStock = async (e: React.FormEvent) => {
    e.preventDefault();
    const sym = newSym.trim().toUpperCase();
    const name = newName.trim();
    if (!sym) return;
    setIsAdding(true);
    try {
      await addStock(sym, name || sym);
      setNewSym('');
      setNewName('');
    } catch (_) {
      // Error toast already displayed by AppContext
    } finally {
      setIsAdding(false);
    }
  };

  const handleCreateWatchlist = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newWatchlistName.trim()) return;
    createWatchlist(newWatchlistName.trim());
    setNewWatchlistName('');
    setIsCreatingWatchlist(false);
  };

  const handleAddStockToWatchlist = (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeWatchlistId) return;
    let sym = newWlStockSym.trim().toUpperCase();
    if (!sym) return;
    if (!sym.includes('.') && !sym.startsWith('^')) {
      sym = `${sym}.NS`;
    }
    addToWatchlist(activeWatchlistId, sym);
    setNewWlStockSym('');
  };

  const handleRenameSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeWatchlistId || !renameValue.trim()) return;
    renameWatchlist(activeWatchlistId, renameValue.trim());
    setIsRenaming(false);
  };

  // Drag resizer for sidebar width
  const startResizing = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startWidth = sidebarWidth;

      const onMouseMove = (moveEvent: MouseEvent) => {
        const delta = moveEvent.clientX - startX;
        const newWidth = Math.max(200, Math.min(600, startWidth + delta));
        setSidebarWidth(newWidth);
      };

      const onMouseUp = () => {
        window.removeEventListener('mousemove', onMouseMove);
        window.removeEventListener('mouseup', onMouseUp);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      };

      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
      window.addEventListener('mousemove', onMouseMove);
      window.addEventListener('mouseup', onMouseUp);
    },
    [sidebarWidth, setSidebarWidth]
  );

  return (
    <aside
      className="sidebar"
      id="sidebar"
      style={{ width: `${sidebarWidth}px`, position: 'relative' }}
    >
      {/* Resizer Handle on right border */}
      <div
        className="sidebar-resize-handle"
        onMouseDown={startResizing}
        title="Drag to resize sidebar"
      />

      {/* Sidebar Header */}
      <div className="sidebar-header">
        <div className="sidebar-tabs">
          <button
            className={`tab-btn ${sidebarTab === 'universe' ? 'active' : ''}`}
            onClick={() => {
              setSidebarTab('universe');
              setGridPage(1);
            }}
            title="Full Stock Universe"
          >
            <span>Universe</span>
            <span className="count-pill" id="count-universe">
              {stocks.length}
            </span>
          </button>
          <button
            className={`tab-btn ${sidebarTab === 'movers' ? 'active' : ''}`}
            onClick={() => {
              setSidebarTab('movers');
              setGridPage(1);
            }}
            title="Top Market Movers"
          >
            <span>Movers</span>
            <span className="count-pill" id="count-movers">
              {filteredMovers.length}
            </span>
          </button>
          <button
            className={`tab-btn ${sidebarTab === 'journal' ? 'active' : ''}`}
            onClick={() => {
              setSidebarTab('journal');
              setGridPage(1);
            }}
            title="Stocks with Analysis Journal"
          >
            <span>Journal</span>
            <span className="count-pill">
              {journalStocks.length}
            </span>
          </button>
          <button
            className={`tab-btn ${sidebarTab === 'watchlist' ? 'active' : ''}`}
            onClick={() => {
              setSidebarTab('watchlist');
              setGridPage(1);
            }}
            title="Custom Watchlists"
          >
            <span>Watchlist</span>
            <span className="count-pill">
              {watchlists.length}
            </span>
          </button>
        </div>

        {/* Watchlist Selector (Watchlist Tab only) */}
        {sidebarTab === 'watchlist' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginTop: '6px' }}>
            <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
              {isRenaming ? (
                <form onSubmit={handleRenameSubmit} style={{ display: 'flex', gap: '4px', flex: 1 }}>
                  <input
                    type="text"
                    className="search-input"
                    value={renameValue}
                    onChange={e => setRenameValue(e.target.value)}
                    autoFocus
                    placeholder="New name..."
                    style={{ flex: 1, padding: '4px 8px', fontSize: '11px' }}
                  />
                  <button type="submit" className="top-action-btn active" style={{ padding: '2px 8px', fontSize: '11px' }}>
                    Save
                  </button>
                  <button
                    type="button"
                    className="top-action-btn"
                    style={{ padding: '2px 6px', fontSize: '11px' }}
                    onClick={() => setIsRenaming(false)}
                  >
                    ✕
                  </button>
                </form>
              ) : (
                <>
                  <select
                    className="search-input"
                    style={{ flex: 1, padding: '4px 6px', fontSize: '11px', cursor: 'pointer' }}
                    value={activeWatchlistId || ''}
                    onChange={e => setActiveWatchlistId(e.target.value || null)}
                  >
                    {watchlists.length === 0 ? (
                      <option value="">No watchlists</option>
                    ) : (
                      watchlists.map(w => (
                        <option key={w.id} value={w.id}>
                          {w.name} ({w.symbols.length})
                        </option>
                      ))
                    )}
                  </select>

                  <button
                    type="button"
                    className="top-action-btn active"
                    style={{ padding: '2px 8px', fontSize: '11px' }}
                    title="Create new watchlist"
                    onClick={() => setIsCreatingWatchlist(prev => !prev)}
                  >
                    + New
                  </button>

                  {currentWatchlist && (
                    <>
                      <button
                        type="button"
                        className="top-action-btn"
                        style={{ padding: '2px 6px', fontSize: '11px' }}
                        title="Rename this watchlist"
                        onClick={() => {
                          setRenameValue(currentWatchlist.name);
                          setIsRenaming(true);
                        }}
                      >
                        ✏️
                      </button>
                      <button
                        type="button"
                        className="top-action-btn"
                        style={{ padding: '2px 6px', fontSize: '11px', color: 'var(--bearish)' }}
                        title="Delete this watchlist"
                        onClick={() => {
                          if (window.confirm(`Delete watchlist "${currentWatchlist.name}"?`)) {
                            deleteWatchlist(currentWatchlist.id);
                          }
                        }}
                      >
                        🗑️
                      </button>
                    </>
                  )}
                </>
              )}
            </div>

            {/* Inline New Watchlist Input */}
            {isCreatingWatchlist && (
              <form onSubmit={handleCreateWatchlist} style={{ display: 'flex', gap: '4px' }}>
                <input
                  type="text"
                  className="search-input"
                  placeholder="Watchlist name (e.g. Breakouts, High Beta)"
                  value={newWatchlistName}
                  onChange={e => setNewWatchlistName(e.target.value)}
                  autoFocus
                  style={{ flex: 1, padding: '4px 8px', fontSize: '11px' }}
                  required
                />
                <button type="submit" className="top-action-btn active" style={{ padding: '2px 8px', fontSize: '11px' }}>
                  Create
                </button>
                <button
                  type="button"
                  className="top-action-btn"
                  style={{ padding: '2px 6px', fontSize: '11px' }}
                  onClick={() => setIsCreatingWatchlist(false)}
                >
                  ✕
                </button>
              </form>
            )}
          </div>
        )}

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
                ? 'Search universe... (/)'
                : sidebarTab === 'movers'
                ? 'Filter movers...'
                : sidebarTab === 'journal'
                ? 'Search journal stocks...'
                : 'Filter watchlist...'
            }
            value={sidebarSearch}
            onChange={e => {
              setSidebarSearch(e.target.value);
              setGridPage(1);
            }}
          />
        </div>

        {/* Movers Filter Controls */}
        {sidebarTab === 'movers' && (
          <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div className="timeframe-group" style={{ height: '24px' }}>
                {[
                  { value: 1.0, label: '1%' },
                  { value: 2.0, label: '2%' },
                  { value: 3.0, label: '3%+' },
                ].map(item => (
                  <button
                    key={item.value}
                    className={`tf-btn ${moversThreshold === item.value ? 'active' : ''}`}
                    style={{ fontSize: '10px', padding: '0 8px' }}
                    onClick={() => {
                      setMoversThreshold(item.value);
                      triggerScan(false, item.value);
                    }}
                  >
                    {item.label}
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
        {/* 1. Universe Tab */}
        {sidebarTab === 'universe' && (
          filteredStocks.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
              No matching stocks found.
            </div>
          ) : (
            filteredStocks.map(stock => {
              const isActive = activeStock?.symbol === stock.symbol;
              const originalIndex = stockIndexMap.get(stock.symbol) ?? 0;

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
        )}

        {/* 2. Top Movers Tab */}
        {sidebarTab === 'movers' && (
          searchedMovers.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
              {sidebarSearch.trim()
                ? `No movers matching "${sidebarSearch}".`
                : `No movers found matching ±${moversThreshold}%.`}
            </div>
          ) : (
            searchedMovers.map(mover => {
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
                      fontSize: '11px',
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
          )
        )}

        {/* 3. Journal Stocks Tab */}
        {sidebarTab === 'journal' && (
          searchedJournalStocks.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
              {journalStocks.length === 0
                ? 'No stocks with analysis notes yet. Press "N" or click "+ Note" on any chart to create a journal note.'
                : `No journal stocks matching "${sidebarSearch}".`}
            </div>
          ) : (
            searchedJournalStocks.map((stock, i) => {
              const isActive = activeStock?.symbol === stock.symbol;

              return (
                <li
                  key={stock.symbol}
                  className={`stock-row ${isActive ? 'active' : ''}`}
                  onClick={() => {
                    selectStock(stock.symbol);
                    setViewMode('single');
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
                    <span className="stock-index-badge">📝 {i + 1}</span>
                    <div style={{ minWidth: 0 }}>
                      <div className="stock-sym">{stock.symbol}</div>
                      <div className="stock-corp-name">{stock.name}</div>
                    </div>
                  </div>
                  <span
                    style={{
                      fontSize: '10px',
                      background: 'var(--accent-subtle)',
                      color: 'var(--accent)',
                      padding: '2px 6px',
                      borderRadius: '4px',
                      fontWeight: 600,
                    }}
                  >
                    Notes
                  </span>
                </li>
              );
            })
          )
        )}

        {/* 4. Watchlists Tab */}
        {sidebarTab === 'watchlist' && (
          !currentWatchlist ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
              <p style={{ marginBottom: '8px' }}>No watchlists created yet.</p>
              <button
                type="button"
                className="top-action-btn active"
                style={{ margin: '0 auto', fontSize: '11px', padding: '4px 12px' }}
                onClick={() => setIsCreatingWatchlist(true)}
              >
                + Create First Watchlist
              </button>
            </div>
          ) : searchedWatchlistStocks.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px' }}>
              {currentWatchlist.symbols.length === 0
                ? 'This watchlist is empty. Add stocks using the form below.'
                : `No stocks matching "${sidebarSearch}".`}
            </div>
          ) : (
            searchedWatchlistStocks.map((stock, i) => {
              const isActive = activeStock?.symbol === stock.symbol;

              return (
                <li
                  key={stock.symbol}
                  className={`stock-row ${isActive ? 'active' : ''}`}
                  onClick={() => {
                    selectStock(stock.symbol);
                    setViewMode('single');
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
                    <span className="stock-index-badge">{i + 1}</span>
                    <div style={{ minWidth: 0 }}>
                      <div className="stock-sym">{stock.symbol}</div>
                      <div className="stock-corp-name">{stock.name}</div>
                    </div>
                  </div>
                  <button
                    className="stock-delete-btn"
                    title={`Remove ${stock.symbol} from "${currentWatchlist.name}"`}
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
                      removeFromWatchlist(currentWatchlist.id, stock.symbol);
                    }}
                  >
                    ✕
                  </button>
                </li>
              );
            })
          )
        )}
      </ul>

      {/* Footer: Universe Tab (Add to Universe) */}
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

      {/* Footer: Watchlist Tab (Add to Watchlist) */}
      {sidebarTab === 'watchlist' && currentWatchlist && (
        <form className="sidebar-footer" onSubmit={handleAddStockToWatchlist} style={{ display: 'flex', gap: '6px' }}>
          <input
            type="text"
            className="search-input"
            placeholder="Add TICKER (e.g. INFY)"
            value={newWlStockSym}
            onChange={e => setNewWlStockSym(e.target.value)}
            style={{ flex: 1 }}
            required
          />
          <button
            type="submit"
            className="top-action-btn active"
            style={{ padding: '4px 10px', fontSize: '11px' }}
          >
            + Add
          </button>
        </form>
      )}
    </aside>
  );
});
