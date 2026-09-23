import type {
  Candle,
  CacheSyncMeta,
  CreateNoteInput,
  MarketStatus,
  ScanResult,
  ScanState,
  Stock,
  StockNote,
  Timeframe,
  UpdateNoteInput,
} from '../types';

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = 'ApiError';
  }
}

async function handleResponse<T>(res: Response): Promise<T> {
  if (!res.ok) {
    let errorMsg = `HTTP Error ${res.status}`;
    try {
      const body = await res.json();
      if (body && body.error) {
        errorMsg = body.error;
      }
    } catch (_) {
      // ignore json parse error
    }
    throw new ApiError(res.status, errorMsg);
  }
  return res.json() as Promise<T>;
}

export const api = {
  // Stocks Universe
  async getStocks(): Promise<Stock[]> {
    const res = await fetch('/api/stocks');
    return handleResponse<Stock[]>(res);
  },

  async addStock(symbol: string, name: string): Promise<Stock> {
    const res = await fetch('/api/stocks', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ symbol: symbol.trim(), name: name.trim() }),
    });
    return handleResponse<Stock>(res);
  },

  async deleteStock(symbol: string): Promise<void> {
    const res = await fetch(`/api/stocks/${encodeURIComponent(symbol)}`, {
      method: 'DELETE',
    });
    if (!res.ok && res.status !== 204) {
      return handleResponse<void>(res);
    }
  },

  // Market Candles
  async getCandles(
    symbol: string,
    timeframe: Timeframe,
    force: boolean = false
  ): Promise<{ candles: Candle[]; cacheHeader: string }> {
    const url = `/api/candles?symbol=${encodeURIComponent(symbol)}&timeframe=${encodeURIComponent(timeframe)}${
      force ? '&force=true' : ''
    }`;
    const res = await fetch(url);
    const cacheHeader = res.headers.get('x-cache') || 'UNKNOWN';
    const candles = await handleResponse<Candle[]>(res);
    return { candles, cacheHeader };
  },

  async getCacheMeta(symbol: string, timeframe: Timeframe): Promise<CacheSyncMeta | null> {
    const res = await fetch(
      `/api/cache/meta?symbol=${encodeURIComponent(symbol)}&timeframe=${encodeURIComponent(timeframe)}`
    );
    if (res.status === 404) return null;
    return handleResponse<CacheSyncMeta>(res);
  },

  // Movers & Scanner
  async getCachedMovers(): Promise<ScanResult | null> {
    const res = await fetch('/api/scan/cached');
    if (res.status === 404) return null;
    return handleResponse<ScanResult>(res);
  },

  async scanMovers(threshold: number = 3.0, force: boolean = false): Promise<ScanResult> {
    const url = `/api/scan?threshold=${threshold}${force ? '&force=true' : ''}`;
    const res = await fetch(url);
    return handleResponse<ScanResult>(res);
  },

  async getScanState(): Promise<ScanState | null> {
    const res = await fetch('/api/scan/state');
    if (res.status === 404) return null;
    return handleResponse<ScanState>(res);
  },

  // Market Status
  async getMarketStatus(): Promise<MarketStatus> {
    const res = await fetch('/api/market/status');
    return handleResponse<MarketStatus>(res);
  },

  // Stock Analysis Notes & Journal
  async getNotes(symbol?: string, status?: string): Promise<StockNote[]> {
    const params = new URLSearchParams();
    if (symbol) params.append('symbol', symbol);
    if (status && status !== 'all') params.append('status', status);
    const query = params.toString() ? `?${params.toString()}` : '';
    const res = await fetch(`/api/notes${query}`);
    return handleResponse<StockNote[]>(res);
  },

  async getNote(id: number): Promise<StockNote> {
    const res = await fetch(`/api/notes/${id}`);
    return handleResponse<StockNote>(res);
  },

  async createNote(input: CreateNoteInput): Promise<StockNote> {
    const res = await fetch('/api/notes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    });
    return handleResponse<StockNote>(res);
  },

  async updateNote(id: number, input: UpdateNoteInput): Promise<StockNote> {
    const res = await fetch(`/api/notes/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    });
    return handleResponse<StockNote>(res);
  },

  async deleteNote(id: number): Promise<void> {
    const res = await fetch(`/api/notes/${id}`, {
      method: 'DELETE',
    });
    if (!res.ok && res.status !== 204) {
      return handleResponse<void>(res);
    }
  },
};
