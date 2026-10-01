import type { Candle } from '../types';

export interface DayChangeResult {
  price: number;
  change: number;
  changePercent: number;
  isPositive: boolean;
  formattedPrice: string;
  formattedChange: string;
}

export interface MinimalQuote {
  price: number;
  prev_close?: number;
  change?: number;
  change_percent?: number;
}

/**
 * Accurately calculates the daily price and percentage change.
 * Prioritizes the official market quote (prev_close) if available.
 * If calculating from candles:
 * - In daily/weekly timeframes, compares against the previous candle (yesterday/last week).
 * - In intraday timeframes (5m, 15m, 30m, 1h), finds the final candle of the previous trading day
 *   (using Asia/Kolkata date boundary). If all candles are from today, uses today's session open.
 */
export function calculateDayChange(
  candles: Candle[],
  timeframe: string = '15m',
  quote?: MinimalQuote | null
): DayChangeResult {
  if (!candles || candles.length === 0) {
    if (quote) {
      const price = quote.price;
      const changePercent = quote.change_percent ?? 0;
      const change = quote.change ?? 0;
      const isPositive = changePercent >= 0;
      return {
        price,
        change,
        changePercent,
        isPositive,
        formattedPrice: `₹${price.toFixed(2)}`,
        formattedChange: `${isPositive ? '+' : ''}${changePercent.toFixed(2)}%`,
      };
    }
    return {
      price: 0,
      change: 0,
      changePercent: 0,
      isPositive: true,
      formattedPrice: '₹--',
      formattedChange: '--',
    };
  }

  const last = candles[candles.length - 1];
  const price = last.close;

  // Case 1: Official quote with valid prev_close available
  if (quote && typeof quote.prev_close === 'number' && quote.prev_close > 0) {
    const prevClose = quote.prev_close;
    const change = price - prevClose;
    const changePercent = (change / prevClose) * 100;
    const isPositive = changePercent >= 0;
    return {
      price,
      change,
      changePercent,
      isPositive,
      formattedPrice: `₹${price.toFixed(2)}`,
      formattedChange: `${isPositive ? '+' : ''}${changePercent.toFixed(2)}%`,
    };
  }

  // Case 2: Only 1 candle available
  if (candles.length === 1) {
    const change = last.close - last.open;
    const changePercent = last.open > 0 ? (change / last.open) * 100 : 0;
    const isPositive = changePercent >= 0;
    return {
      price,
      change,
      changePercent,
      isPositive,
      formattedPrice: `₹${price.toFixed(2)}`,
      formattedChange: `${isPositive ? '+' : ''}${changePercent.toFixed(2)}%`,
    };
  }

  // Case 3: Daily or Weekly timeframe -> previous candle is the baseline
  if (timeframe === '1d' || timeframe === '1w') {
    const prev = candles[candles.length - 2];
    const prevClose = prev.close;
    const change = price - prevClose;
    const changePercent = prevClose > 0 ? (change / prevClose) * 100 : 0;
    const isPositive = changePercent >= 0;
    return {
      price,
      change,
      changePercent,
      isPositive,
      formattedPrice: `₹${price.toFixed(2)}`,
      formattedChange: `${isPositive ? '+' : ''}${changePercent.toFixed(2)}%`,
    };
  }

  // Case 4: Intraday timeframes (5m, 15m, 30m, 1h)
  // Locate the closing candle of the previous trading day (IST date boundary)
  try {
    const lastDateStr = new Date(last.timestamp * 1000).toLocaleDateString('en-CA', {
      timeZone: 'Asia/Kolkata',
    });

    for (let i = candles.length - 2; i >= 0; i--) {
      const candleDateStr = new Date(candles[i].timestamp * 1000).toLocaleDateString('en-CA', {
        timeZone: 'Asia/Kolkata',
      });
      if (candleDateStr !== lastDateStr) {
        const prevClose = candles[i].close;
        const change = price - prevClose;
        const changePercent = prevClose > 0 ? (change / prevClose) * 100 : 0;
        const isPositive = changePercent >= 0;
        return {
          price,
          change,
          changePercent,
          isPositive,
          formattedPrice: `₹${price.toFixed(2)}`,
          formattedChange: `${isPositive ? '+' : ''}${changePercent.toFixed(2)}%`,
        };
      }
    }
  } catch (_) {
    // If timezone parsing fails, fall through to session open
  }

  // Fallback: If all candles belong to today's session, compare against session open
  const sessionOpen = candles[0].open;
  const change = price - sessionOpen;
  const changePercent = sessionOpen > 0 ? (change / sessionOpen) * 100 : 0;
  const isPositive = changePercent >= 0;
  return {
    price,
    change,
    changePercent,
    isPositive,
    formattedPrice: `₹${price.toFixed(2)}`,
    formattedChange: `${isPositive ? '+' : ''}${changePercent.toFixed(2)}%`,
  };
}
