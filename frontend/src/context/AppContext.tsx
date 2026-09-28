import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import type {
  DrawerTab,
  GridScope,
  JournalFilterScope,
  JournalFilterStatus,
  MarketStatus,
  ScanResult,
  SidebarTab,
  Stock,
  StockMover,
  StockNote,
  Timeframe,
  ViewMode,
  Candle,
  Watchlist,
} from '../types';
import { api } from '../api/client';

interface AppContextValue {
  // Universe & Stocks
  stocks: Stock[];
  currentIndex: number;
  activeStock: Stock | null;
  selectStock: (symbolOrIndex: string | number) => void;
  nextStock: () => void;
  prevStock: () => void;
  reloadStocks: () => Promise<Stock[] | null>;
  addStock: (symbol: string, name: string) => Promise<void>;
  deleteStock: (symbol: string) => Promise<void>;

  // Timeframe & View Mode
  timeframe: Timeframe;
  setTimeframe: (tf: Timeframe) => void;
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;

  // Movers
  movers: ScanResult | null;
  moversThreshold: number;
  setMoversThreshold: (val: number) => void;
  moversFilter: 'all' | 'gainers' | 'losers';
  setMoversFilter: (filter: 'all' | 'gainers' | 'losers') => void;
  filteredMovers: StockMover[];
  triggerScan: (force?: boolean, thresholdOverride?: number) => Promise<void>;

  // Sidebar
  sidebarTab: SidebarTab;
  setSidebarTab: (tab: SidebarTab) => void;
  sidebarSearch: string;
  setSidebarSearch: (q: string) => void;

  // Watchlists
  watchlists: Watchlist[];
  activeWatchlistId: string | null;
  setActiveWatchlistId: (id: string | null) => void;
  createWatchlist: (name: string) => void;
  deleteWatchlist: (id: string) => void;
  renameWatchlist: (id: string, name: string) => void;
  addToWatchlist: (watchlistId: string, symbol: string) => void;
  removeFromWatchlist: (watchlistId: string, symbol: string) => void;
  toggleWatchlistSymbol: (watchlistId: string, symbol: string) => void;

  // Journal Stocks (auto-populated from notes)
  journalStocks: Stock[];
  refreshJournalStocks: () => Promise<void>;

  // Sidebar panel width (resizable)
  sidebarWidth: number;
  setSidebarWidth: (w: number) => void;
  drawerWidth: number;
  setDrawerWidth: (w: number) => void;

  // Grid
  gridScope: GridScope;
  setGridScope: (scope: GridScope) => void;
  gridSearch: string;
  setGridSearch: (q: string) => void;
  gridPage: number;
  setGridPage: (p: number | ((prev: number) => number)) => void;
  gridPageSize: number;

  // Intel Drawer
  isDrawerOpen: boolean;
  toggleDrawer: () => void;
  drawerTab: DrawerTab;
  setDrawerTab: (tab: DrawerTab) => void;

  // Journal Filter State
  journalFilterScope: JournalFilterScope;
  setJournalFilterScope: (scope: JournalFilterScope) => void;
  journalFilterStatus: JournalFilterStatus;
  setJournalFilterStatus: (status: JournalFilterStatus) => void;

  // Market Status & Cache Latency
  marketStatus: MarketStatus;
  marketStatusLoaded: boolean;
  cacheLatency: string;
  setCacheLatency: (latency: string) => void;
  chartCacheHeader: string;
  setChartCacheHeader: (header: string) => void;
  latestPrice: number;
  latestCandleTimestamp: number | null;
  setLatestCandleInfo: (price: number, ts: number | null) => void;
  candles: Candle[];
  setCandles: (candles: Candle[]) => void;

  // Modals
  isNoteModalOpen: boolean;
  editingNote: StockNote | null;
  openNoteModal: (note?: StockNote) => void;
  closeNoteModal: () => void;

  isAuditModalOpen: boolean;
  auditNote: StockNote | null;
  openAuditModal: (note: StockNote) => void;
  closeAuditModal: () => void;

  isShortcutsOpen: boolean;
  openShortcuts: () => void;
  closeShortcuts: () => void;

  isCommandPaletteOpen: boolean;
  openCommandPalette: () => void;
  closeCommandPalette: () => void;

  // Force chart refresh trigger
  forceRefreshCounter: number;
  triggerForceRefresh: () => void;

  // Theme & Zoom
  isDark: boolean;
  toggleTheme: () => void;
  zoom: number;
  setZoom: (z: number | ((prev: number) => number)) => void;
  zoomIn: () => void;
  zoomOut: () => void;

  // Toast
  toast: string | null;
  showToast: (msg: string) => void;
}

const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: { children: React.ReactNode }) {
  // Stocks state
  const [stocks, setStocks] = useState<Stock[]>([]);
  const [currentIndex, setCurrentIndex] = useState<number>(0);

  // Active timeframe & view mode
  const [timeframe, setTimeframe] = useState<Timeframe>('15m');
  const [viewMode, setViewMode] = useState<ViewMode>('single');

  // Movers
  const [movers, setMovers] = useState<ScanResult | null>(null);
  const [moversThreshold, setMoversThreshold] = useState<number>(3.0);
  const [moversFilter, setMoversFilter] = useState<'all' | 'gainers' | 'losers'>('all');

  // Sidebar
  const [sidebarTab, setSidebarTab] = useState<SidebarTab>('universe');
  const [sidebarSearch, setSidebarSearch] = useState<string>('');

  // Watchlists (persisted in localStorage)
  const [watchlists, setWatchlists] = useState<Watchlist[]>(() => {
    try {
      const saved = localStorage.getItem('mw_watchlists');
      return saved ? JSON.parse(saved) : [];
    } catch { return []; }
  });
  const [activeWatchlistId, setActiveWatchlistId] = useState<string | null>(() => {
    return localStorage.getItem('mw_active_watchlist') || null;
  });

  useEffect(() => {
    localStorage.setItem('mw_watchlists', JSON.stringify(watchlists));
  }, [watchlists]);

  useEffect(() => {
    if (activeWatchlistId) {
      localStorage.setItem('mw_active_watchlist', activeWatchlistId);
    } else {
      localStorage.removeItem('mw_active_watchlist');
    }
  }, [activeWatchlistId]);

  // Journal Stocks (auto-populated from notes API)
  const [journalStocks, setJournalStocks] = useState<Stock[]>([]);
  const journalFetchGeneration = useRef(0);

  // Resizable panel widths (persisted in localStorage)
  const [sidebarWidth, setSidebarWidth] = useState<number>(() => {
    const saved = localStorage.getItem('mw_sidebar_width');
    const parsed = saved ? parseInt(saved, 10) : 300;
    return Number.isFinite(parsed) && parsed >= 180 && parsed <= 600 ? parsed : 300;
  });
  const [drawerWidth, setDrawerWidth] = useState<number>(() => {
    const saved = localStorage.getItem('mw_drawer_width');
    const parsed = saved ? parseInt(saved, 10) : 380;
    return Number.isFinite(parsed) && parsed >= 250 && parsed <= 700 ? parsed : 380;
  });

  useEffect(() => {
    localStorage.setItem('mw_sidebar_width', String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    localStorage.setItem('mw_drawer_width', String(drawerWidth));
  }, [drawerWidth]);

  // Grid
  const [gridScope, setGridScope] = useState<GridScope>('universe');
  const [gridSearch, setGridSearch] = useState<string>('');
  const [gridPage, setGridPage] = useState<number>(1);
  const gridPageSize = 24;

  // Drawer
  const [isDrawerOpen, setIsDrawerOpen] = useState<boolean>(true);
  const [drawerTab, setDrawerTab] = useState<DrawerTab>('ai');

  // Journal Filters
  const [journalFilterScope, setJournalFilterScope] = useState<JournalFilterScope>('stock');
  const [journalFilterStatus, setJournalFilterStatus] = useState<JournalFilterStatus>('all');

  // Market Status & Latency
  const [marketStatus, setMarketStatus] = useState<MarketStatus>({ is_open: false, is_trading_day: false });
  const [marketStatusLoaded, setMarketStatusLoaded] = useState(false);
  const [cacheLatency, setCacheLatency] = useState<string>('--');
  const [chartCacheHeader, setChartCacheHeader] = useState<string>('UNKNOWN');
  const [latestPrice, setLatestPrice] = useState<number>(0);
  const [latestCandleTimestamp, setLatestCandleTimestamp] = useState<number | null>(null);
  const [candles, setCandles] = useState<Candle[]>([]);

  const setLatestCandleInfo = useCallback((price: number, ts: number | null) => {
    setLatestPrice(price);
    setLatestCandleTimestamp(ts);
  }, []);

  // Modals
  const [isNoteModalOpen, setIsNoteModalOpen] = useState<boolean>(false);
  const [editingNote, setEditingNote] = useState<StockNote | null>(null);

  const [isAuditModalOpen, setIsAuditModalOpen] = useState<boolean>(false);
  const [auditNote, setAuditNote] = useState<StockNote | null>(null);

  const [isShortcutsOpen, setIsShortcutsOpen] = useState<boolean>(false);
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState<boolean>(false);

  // Theme state (persisted in localStorage, default dark)
  const [isDark, setIsDark] = useState<boolean>(() => {
    const saved = localStorage.getItem('mw_theme');
    return saved ? saved === 'dark' : true;
  });

  useEffect(() => {
    document.body.classList.toggle('dark-theme', isDark);
    localStorage.setItem('mw_theme', isDark ? 'dark' : 'light');
  }, [isDark]);

  const toggleTheme = useCallback(() => {
    setIsDark(prev => !prev);
  }, []);

  // Zoom state (candle barSpacing, persisted in localStorage, default 2.5)
  const [zoom, setZoom] = useState<number>(() => {
    const saved = localStorage.getItem('mw-zoom');
    const parsed = saved ? parseFloat(saved) : 2.5;
    return Number.isFinite(parsed) && parsed >= 0.5 && parsed <= 10 ? parsed : 2.5;
  });

  useEffect(() => {
    localStorage.setItem('mw-zoom', String(zoom));
  }, [zoom]);

  const zoomIn = useCallback(() => {
    setZoom(prev => Math.min(10, Math.round((prev + 0.5) * 10) / 10));
  }, []);

  const zoomOut = useCallback(() => {
    setZoom(prev => Math.max(0.5, Math.round((prev - 0.5) * 10) / 10));
  }, []);

  // Toast
  const [toast, setToast] = useState<string | null>(null);
  const [forceRefreshCounter, setForceRefreshCounter] = useState<number>(0);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => {
      setToast(prev => (prev === msg ? null : prev));
    }, 2500);
  }, []);

  // Load stocks (returns the fresh list so callers can act on it without
  // reading the still-stale `stocks` closure).
  const reloadStocks = useCallback(async () => {
    try {
      const data = await api.getStocks();
      setStocks(data);
      return data;
    } catch (err: any) {
      showToast(`Failed to load stocks: ${err.message}`);
      return null;
    }
  }, [showToast]);

  // Load cached movers
  const loadCachedMovers = useCallback(async () => {
    try {
      const data = await api.getCachedMovers();
      if (data) setMovers(data);
    } catch (_) {
      // ignore
    }
  }, []);

  // Update market status
  const updateMarketStatus = useCallback(async () => {
    try {
      const status = await api.getMarketStatus();
      setMarketStatus(status);
      setMarketStatusLoaded(true);
    } catch (_) {
      // ignore
    }
  }, []);

  // Refresh stocks that have journal notes
  const refreshJournalStocks = useCallback(async () => {
    const gen = ++journalFetchGeneration.current;
    try {
      const notes = await api.getNotes();
      if (gen !== journalFetchGeneration.current) return;
      const seen = new Set<string>();
      const list: Stock[] = [];
      for (const n of notes) {
        const sym = n.symbol.toUpperCase();
        if (!seen.has(sym)) {
          seen.add(sym);
          const found = stocks.find(s => s.symbol.toUpperCase() === sym);
          list.push(found || { symbol: sym, name: sym });
        }
      }
      setJournalStocks(list);
    } catch (_) {
      // ignore
    }
  }, [stocks]);

  // Initial load
  useEffect(() => {
    reloadStocks();
    loadCachedMovers();
    updateMarketStatus();
    const interval = setInterval(updateMarketStatus, 60000);
    return () => clearInterval(interval);
  }, [reloadStocks, loadCachedMovers, updateMarketStatus]);

  // Watchlist active ID fallback
  useEffect(() => {
    if ((!activeWatchlistId || !watchlists.some(w => w.id === activeWatchlistId)) && watchlists.length > 0) {
      setActiveWatchlistId(watchlists[0].id);
    }
  }, [watchlists, activeWatchlistId]);

  // Fetch journal stocks on change
  useEffect(() => {
    refreshJournalStocks();
  }, [refreshJournalStocks, forceRefreshCounter]);

  // Active stock derivation
  const activeStock = useMemo(() => {
    if (stocks.length === 0) return null;
    return stocks[currentIndex] || stocks[0];
  }, [stocks, currentIndex]);

  // Select stock by symbol or index
  const selectStock = useCallback(
    (symbolOrIndex: string | number) => {
      if (typeof symbolOrIndex === 'number') {
        if (symbolOrIndex >= 0 && symbolOrIndex < stocks.length) {
          setCurrentIndex(symbolOrIndex);
        }
      } else {
        const idx = stocks.findIndex(s => s.symbol.toUpperCase() === symbolOrIndex.toUpperCase());
        if (idx !== -1) {
          setCurrentIndex(idx);
        }
      }
    },
    [stocks]
  );

  // Filtered movers, prioritizing all_quotes for instant client-side threshold filtering
  const filteredMovers = useMemo(() => {
    let list: StockMover[] = [];
    if (Array.isArray(movers?.all_quotes) && movers!.all_quotes.length > 0) {
      list = movers!.all_quotes.filter(m => Math.abs(m.change_percent) >= moversThreshold);
    } else if (Array.isArray(movers?.movers)) {
      list = movers!.movers.filter(m => Math.abs(m.change_percent) >= moversThreshold);
    }
    list = [...list].sort((a, b) => Math.abs(b.change_percent) - Math.abs(a.change_percent));
    if (moversFilter === 'gainers') return list.filter(m => m.change_percent >= 0);
    if (moversFilter === 'losers') return list.filter(m => m.change_percent < 0);
    return list;
  }, [movers, moversFilter, moversThreshold]);

  // Get the list of symbols for the currently active sidebar tab, respecting any active search query
  const activeTabSymbols = useMemo((): string[] => {
    const q = sidebarSearch.trim().toLowerCase();
    switch (sidebarTab) {
      case 'movers': {
        const list = q
          ? filteredMovers.filter(
              m => m.symbol.toLowerCase().includes(q) || (m.name && m.name.toLowerCase().includes(q))
            )
          : filteredMovers;
        return list.map(m => m.symbol);
      }
      case 'journal': {
        const list = q
          ? journalStocks.filter(
              s => s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
            )
          : journalStocks;
        return list.map(s => s.symbol);
      }
      case 'watchlist': {
        if (!activeWatchlistId) return [];
        const wl = watchlists.find(w => w.id === activeWatchlistId);
        if (!wl) return [];
        const syms = wl.symbols;
        if (!q) return syms;
        return syms.filter(sym => sym.toLowerCase().includes(q));
      }
      default: { // 'universe'
        const list = q
          ? stocks.filter(
              s => s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
            )
          : stocks;
        return list.map(s => s.symbol);
      }
    }
  }, [sidebarTab, sidebarSearch, stocks, filteredMovers, journalStocks, activeWatchlistId, watchlists]);

  const nextStock = useCallback(() => {
    const syms = activeTabSymbols;
    if (syms.length === 0) return;
    const currentSym = activeStock?.symbol?.toUpperCase() || '';
    const idx = syms.findIndex(s => s.toUpperCase() === currentSym);
    const nextIdx = idx === -1 ? 0 : (idx + 1) % syms.length;
    selectStock(syms[nextIdx]);
  }, [activeTabSymbols, activeStock, selectStock]);

  const prevStock = useCallback(() => {
    const syms = activeTabSymbols;
    if (syms.length === 0) return;
    const currentSym = activeStock?.symbol?.toUpperCase() || '';
    const idx = syms.findIndex(s => s.toUpperCase() === currentSym);
    const prevIdx = idx === -1 ? 0 : (idx - 1 + syms.length) % syms.length;
    selectStock(syms[prevIdx]);
  }, [activeTabSymbols, activeStock, selectStock]);

  const addStock = useCallback(
    async (symbol: string, name: string) => {
      try {
        const added = await api.addStock(symbol, name);
        showToast(`Added ${added.symbol}`);
        const fresh = await reloadStocks();
        // Select by index in the reloaded list: `selectStock` closes over the
        // pre-reload `stocks` array and can miss the new entry.
        if (fresh) {
          const idx = fresh.findIndex(
            s => s.symbol.toUpperCase() === added.symbol.toUpperCase()
          );
          if (idx !== -1) selectStock(idx);
        }
      } catch (err: any) {
        showToast(`Error: ${err.message}`);
        throw err;
      }
    },
    [reloadStocks, selectStock, showToast]
  );

  const deleteStock = useCallback(
    async (symbol: string) => {
      if (!window.confirm(`Remove ${symbol} from universe?`)) return;
      try {
        await api.deleteStock(symbol);
        showToast(`Removed ${symbol}`);
        // Case-insensitive: the backend deletes case-insensitively, so an
        // exact-case filter would leave the row in local state.
        const upper = symbol.toUpperCase();
        const nextList = stocks.filter(s => s.symbol.toUpperCase() !== upper);
        setStocks(nextList);
        setMovers(prev => {
          if (!prev) return prev;
          const drop = (m: StockMover) => m.symbol.toUpperCase() !== upper;
          return {
            ...prev,
            movers: prev.movers.filter(drop),
            all_quotes: prev.all_quotes.filter(drop),
          };
        });
        setCurrentIndex(prev => Math.min(prev, Math.max(0, nextList.length - 1)));
      } catch (err: any) {
        showToast(`Error: ${err.message}`);
      }
    },
    [stocks, showToast]
  );

  // Watchlists operations
  const createWatchlist = useCallback(
    (name: string) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      const newWl: Watchlist = {
        id: `wl_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`,
        name: trimmed,
        symbols: [],
        createdAt: Date.now(),
      };
      setWatchlists(prev => [...prev, newWl]);
      setActiveWatchlistId(newWl.id);
      showToast(`Created watchlist "${trimmed}"`);
    },
    [showToast]
  );

  const deleteWatchlist = useCallback(
    (id: string) => {
      setWatchlists(prev => {
        const target = prev.find(w => w.id === id);
        if (target) showToast(`Deleted watchlist "${target.name}"`);
        return prev.filter(w => w.id !== id);
      });
      setActiveWatchlistId(prev => {
        if (prev === id) {
          const remaining = watchlists.filter(w => w.id !== id);
          return remaining.length > 0 ? remaining[0].id : null;
        }
        return prev;
      });
    },
    [showToast, watchlists]
  );

  const renameWatchlist = useCallback(
    (id: string, name: string) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      setWatchlists(prev => prev.map(w => (w.id === id ? { ...w, name: trimmed } : w)));
      showToast(`Renamed watchlist to "${trimmed}"`);
    },
    [showToast]
  );

  const addToWatchlist = useCallback(
    (watchlistId: string, symbol: string) => {
      const sym = symbol.trim().toUpperCase();
      if (!sym) return;
      setWatchlists(prev =>
        prev.map(w => {
          if (w.id === watchlistId) {
            if (w.symbols.includes(sym)) return w;
            return { ...w, symbols: [...w.symbols, sym] };
          }
          return w;
        })
      );
      showToast(`Added ${sym} to watchlist`);
    },
    [showToast]
  );

  const removeFromWatchlist = useCallback(
    (watchlistId: string, symbol: string) => {
      const sym = symbol.trim().toUpperCase();
      setWatchlists(prev =>
        prev.map(w => {
          if (w.id === watchlistId) {
            return { ...w, symbols: w.symbols.filter(s => s !== sym) };
          }
          return w;
        })
      );
      showToast(`Removed ${sym} from watchlist`);
    },
    [showToast]
  );

  const toggleWatchlistSymbol = useCallback((watchlistId: string, symbol: string) => {
    const sym = symbol.trim().toUpperCase();
    if (!sym) return;
    setWatchlists(prev =>
      prev.map(w => {
        if (w.id === watchlistId) {
          const exists = w.symbols.includes(sym);
          return {
            ...w,
            symbols: exists ? w.symbols.filter(s => s !== sym) : [...w.symbols, sym],
          };
        }
        return w;
      })
    );
  }, []);


  const triggerScan = useCallback(
    async (force: boolean = false, thresholdOverride?: number) => {
      // Use the explicit threshold when provided: `setMoversThreshold` is
      // async, so reading state right after setting it scans stale.
      const threshold = thresholdOverride ?? moversThreshold;
      if (!Number.isFinite(threshold) || threshold <= 0 || threshold > 100) {
        showToast(`Invalid threshold ${threshold}: must be in (0, 100]`);
        return;
      }
      try {
        showToast('Scanning market movers...');
        const res = await api.scanMovers(threshold, force);
        setMovers(res);
        const failed =
          res.failed_count && res.failed_count > 0 ? `, ${res.failed_count} failed` : '';
        showToast(`Scanned ${res.total_scanned} stocks (${res.movers_count} movers${failed})`);
      } catch (err: any) {
        showToast(`Scan failed: ${err.message}`);
      }
    },
    [moversThreshold, showToast]
  );

  // Drawer
  const toggleDrawer = useCallback(() => {
    setIsDrawerOpen(prev => !prev);
  }, []);

  // Modals
  const openNoteModal = useCallback((note?: StockNote) => {
    setEditingNote(note || null);
    setIsNoteModalOpen(true);
  }, []);

  const closeNoteModal = useCallback(() => {
    setIsNoteModalOpen(false);
    setEditingNote(null);
  }, []);

  const openAuditModal = useCallback((note: StockNote) => {
    setAuditNote(note);
    setIsAuditModalOpen(true);
  }, []);

  const closeAuditModal = useCallback(() => {
    setIsAuditModalOpen(false);
    setAuditNote(null);
  }, []);

  const openShortcuts = useCallback(() => setIsShortcutsOpen(true), []);
  const closeShortcuts = useCallback(() => setIsShortcutsOpen(false), []);

  const openCommandPalette = useCallback(() => setIsCommandPaletteOpen(true), []);
  const closeCommandPalette = useCallback(() => setIsCommandPaletteOpen(false), []);

  const triggerForceRefresh = useCallback(() => {
    setForceRefreshCounter(c => c + 1);
  }, []);

  const value: AppContextValue = useMemo(
    () => ({
      stocks,
      currentIndex,
      activeStock,
      selectStock,
      nextStock,
      prevStock,
      reloadStocks,
      addStock,
      deleteStock,

      timeframe,
      setTimeframe,
      viewMode,
      setViewMode,

      movers,
      moversThreshold,
      setMoversThreshold,
      moversFilter,
      setMoversFilter,
      filteredMovers,
      triggerScan,

      sidebarTab,
      setSidebarTab,
      sidebarSearch,
      setSidebarSearch,

      // Watchlists
      watchlists,
      activeWatchlistId,
      setActiveWatchlistId,
      createWatchlist,
      deleteWatchlist,
      renameWatchlist,
      addToWatchlist,
      removeFromWatchlist,
      toggleWatchlistSymbol,

      // Journal Stocks
      journalStocks,
      refreshJournalStocks,

      // Panel Dimensions
      sidebarWidth,
      setSidebarWidth,
      drawerWidth,
      setDrawerWidth,

      gridScope,
      setGridScope,
      gridSearch,
      setGridSearch,
      gridPage,
      setGridPage,
      gridPageSize,

      isDrawerOpen,
      toggleDrawer,
      drawerTab,
      setDrawerTab,

      journalFilterScope,
      setJournalFilterScope,
      journalFilterStatus,
      setJournalFilterStatus,

      marketStatus,
      marketStatusLoaded,
      cacheLatency,
      setCacheLatency,
      chartCacheHeader,
      setChartCacheHeader,
      latestPrice,
      latestCandleTimestamp,
      setLatestCandleInfo,
      candles,
      setCandles,

      isNoteModalOpen,
      editingNote,
      openNoteModal,
      closeNoteModal,

      isAuditModalOpen,
      auditNote,
      openAuditModal,
      closeAuditModal,

      isShortcutsOpen,
      openShortcuts,
      closeShortcuts,

      isCommandPaletteOpen,
      openCommandPalette,
      closeCommandPalette,

      forceRefreshCounter,
      triggerForceRefresh,

      // Theme & Zoom
      isDark,
      toggleTheme,
      zoom,
      setZoom,
      zoomIn,
      zoomOut,

      toast,
      showToast,
    }),
    [
      stocks,
      currentIndex,
      activeStock,
      selectStock,
      nextStock,
      prevStock,
      reloadStocks,
      addStock,
      deleteStock,
      timeframe,
      setTimeframe,
      viewMode,
      setViewMode,
      movers,
      moversThreshold,
      setMoversThreshold,
      moversFilter,
      setMoversFilter,
      filteredMovers,
      triggerScan,
      sidebarTab,
      setSidebarTab,
      sidebarSearch,
      setSidebarSearch,
      watchlists,
      activeWatchlistId,
      setActiveWatchlistId,
      createWatchlist,
      deleteWatchlist,
      renameWatchlist,
      addToWatchlist,
      removeFromWatchlist,
      toggleWatchlistSymbol,
      journalStocks,
      refreshJournalStocks,
      sidebarWidth,
      setSidebarWidth,
      drawerWidth,
      setDrawerWidth,
      gridScope,
      setGridScope,
      gridSearch,
      setGridSearch,
      gridPage,
      setGridPage,
      gridPageSize,
      isDrawerOpen,
      toggleDrawer,
      drawerTab,
      setDrawerTab,
      journalFilterScope,
      setJournalFilterScope,
      journalFilterStatus,
      setJournalFilterStatus,
      marketStatus,
      marketStatusLoaded,
      cacheLatency,
      setCacheLatency,
      chartCacheHeader,
      setChartCacheHeader,
      latestPrice,
      latestCandleTimestamp,
      setLatestCandleInfo,
      candles,
      setCandles,
      isNoteModalOpen,
      editingNote,
      openNoteModal,
      closeNoteModal,
      isAuditModalOpen,
      auditNote,
      openAuditModal,
      closeAuditModal,
      isShortcutsOpen,
      openShortcuts,
      closeShortcuts,
      isCommandPaletteOpen,
      openCommandPalette,
      closeCommandPalette,
      forceRefreshCounter,
      triggerForceRefresh,
      isDark,
      toggleTheme,
      zoom,
      setZoom,
      zoomIn,
      zoomOut,
      toast,
      showToast,
    ]
  );

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error('useApp must be used within an AppProvider');
  }
  return context;
}
