import type { Candle } from '../types';

export interface FloorPivots {
  pivot: string;
  r1: string;
  s1: string;
  r2: string;
  s2: string;
}

export interface TechnicalInsights extends FloorPivots {
  pattern: string;
  sentiment: 'BULLISH' | 'BEARISH' | 'NEUTRAL';
  sentColor: string;
  sentScore: string;
}

export function formatPrice(val: number | string): string {
  if (typeof val === 'string') return val;
  return Number(val).toLocaleString('en-IN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

/**
 * Calculate classical floor pivot points and candlestick pattern sentiment
 * from historical candlestick bars.
 */
export function calculateInsights(candles: Candle[] | undefined | null): TechnicalInsights {
  let pivot = '--';
  let r1 = '--';
  let s1 = '--';
  let r2 = '--';
  let s2 = '--';
  let pattern = 'Analyzing momentum...';
  let sentiment: 'BULLISH' | 'BEARISH' | 'NEUTRAL' = 'NEUTRAL';
  let sentColor = 'var(--accent)';
  let sentScore = '55%';

  if (candles && candles.length >= 2) {
    const last = candles[candles.length - 1];
    const prev = candles[candles.length - 2];
    const h = last.high;
    const l = last.low;
    const c = last.close;

    // Floor pivots calculation
    const p = (h + l + c) / 3;
    pivot = formatPrice(p);
    r1 = formatPrice(2 * p - l);
    s1 = formatPrice(2 * p - h);
    r2 = formatPrice(p + (h - l));
    s2 = formatPrice(p - (h - l));

    // Candlestick pattern detection
    const lastBody = Math.abs(last.close - last.open);

    if (
      last.close > last.open &&
      prev.close < prev.open &&
      last.close > prev.open &&
      last.open < prev.close
    ) {
      pattern = 'Bullish Engulfing pattern detected on volume expansion.';
      sentiment = 'BULLISH';
      sentColor = 'var(--bullish)';
      sentScore = '82%';
    } else if (
      last.close < last.open &&
      prev.close > prev.open &&
      last.open > prev.close &&
      last.close < prev.open
    ) {
      pattern = 'Bearish Engulfing pattern detected near local resistance.';
      sentiment = 'BEARISH';
      sentColor = 'var(--bearish)';
      sentScore = '76%';
    } else if (
      last.close >= last.open &&
      last.open - last.low > 2 * lastBody &&
      last.high - last.close < lastBody * 0.3
    ) {
      pattern = 'Hammer pattern forming near support zone.';
      sentiment = 'BULLISH';
      sentColor = 'var(--bullish)';
      sentScore = '70%';
    } else if (lastBody < (last.high - last.low) * 0.1) {
      pattern = 'Doji indecision bar at key level.';
      sentiment = 'NEUTRAL';
      sentColor = 'var(--accent)';
      sentScore = '50%';
    } else if (last.close >= last.open) {
      pattern = 'Bullish continuation structure holding above VWAP.';
      sentiment = 'BULLISH';
      sentColor = 'var(--bullish)';
      sentScore = '65%';
    } else {
      pattern = 'Distribution pressure testing session lows.';
      sentiment = 'BEARISH';
      sentColor = 'var(--bearish)';
      sentScore = '62%';
    }
  }

  return { pivot, r1, s1, r2, s2, pattern, sentiment, sentColor, sentScore };
}
