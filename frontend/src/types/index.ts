export type Timeframe = '5m' | '15m' | '30m' | '1h' | '1d' | '1w';
export type ViewMode = 'single' | 'grid';
export type GridScope = 'universe' | 'movers';
export type SidebarTab = 'universe' | 'movers';
export type DrawerTab = 'ai' | 'journal' | 'fundamentals' | 'system';
export type NoteStatus = 'open' | 'validated' | 'invalidated' | 'cancelled';
export type JournalFilterScope = 'stock' | 'all';
export type JournalFilterStatus = 'all' | 'open' | 'validated' | 'invalidated';

export interface Stock {
  symbol: string;
  name: string;
}

export interface StockMover {
  symbol: string;
  name?: string;
  price: number;
  change_percent: number;
  previous_close?: number;
  sparkline: number[];
  is_positive: boolean;
}

export interface Candle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface StockNote {
  id: number;
  symbol: string;
  timeframe: string;
  candle_timestamp: number | null;
  price_at_note: number;
  title: string;
  content: string;
  tags: string;
  status: NoteStatus;
  target_price: number | null;
  stop_loss: number | null;
  outcome_note: string | null;
  verified_at: number | null;
  reference_note_ids: number[];
  created_at: number;
  updated_at: number;
}

export interface CreateNoteInput {
  symbol: string;
  timeframe: string;
  candle_timestamp?: number | null;
  price_at_note: number;
  title: string;
  content: string;
  tags?: string;
  target_price?: number | null;
  stop_loss?: number | null;
  reference_note_ids?: number[];
}

export interface UpdateNoteInput {
  title?: string;
  content?: string;
  tags?: string;
  status?: NoteStatus;
  target_price?: number | null;
  stop_loss?: number | null;
  outcome_note?: string | null;
  verified_at?: number | null;
  reference_note_ids?: number[];
}

export interface MarketStatus {
  is_open: boolean;
  is_trading_day: boolean;
}

export interface CacheSyncMeta {
  symbol: string;
  timeframe: string;
  first_candle_ts: number;
  last_candle_ts: number;
  candle_count: number;
  last_synced_at: number;
  has_gap: boolean;
}

export interface ScanResult {
  scan_time: string;
  threshold_used: number;
  stocks_scanned: number;
  gainers: StockMover[];
  losers: StockMover[];
}

export interface ScanState {
  last_scan_result: ScanResult | null;
  last_morning_scan_date: string | null;
  last_evening_scan_date: string | null;
}
