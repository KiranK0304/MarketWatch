import React, { createContext, useContext, useState, useEffect, useCallback, useMemo } from 'react';
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

  // Initial load
  useEffect(() => {
    reloadStocks();
    loadCachedMovers();
    updateMarketStatus();
    const interval = setInterval(updateMarketStatus, 60000);
    return () => clearInterval(interval);
  }, [reloadStocks, loadCachedMovers, updateMarketStatus]);

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

  const nextStock = useCallback(() => {
    if (stocks.length === 0) return;
    setCurrentIndex(prev => (prev + 1) % stocks.length);
  }, [stocks.length]);

  const prevStock = useCallback(() => {
    if (stocks.length === 0) return;
    setCurrentIndex(prev => (prev - 1 + stocks.length) % stocks.length);
  }, [stocks.length]);

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

  // Filtered movers, derived from the backend `movers` list. Older cached
  // payloads (pre-contract-fix) lack `movers`, so fall back to empty.
  const filteredMovers = useMemo(() => {
    const list = Array.isArray(movers?.movers) ? movers!.movers : [];
    if (moversFilter === 'gainers') return list.filter(m => m.change_percent >= 0);
    if (moversFilter === 'losers') return list.filter(m => m.change_percent < 0);
    return list;
  }, [movers, moversFilter]);

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

  const value: AppContextValue = {
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

    toast,
    showToast,
  };

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error('useApp must be used within an AppProvider');
  }
  return context;
}
