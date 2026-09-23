import React, { useEffect, useRef, useState, useCallback } from 'react';
import { createChart, IChartApi, ISeriesApi, CandlestickData, HistogramData, Time } from 'lightweight-charts';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import type { Candle, StockNote } from '../../types';

interface FocusChartProps {
  onPriceUpdate?: (price: string, change: { text: string; isPositive: boolean }) => void;
}

export const FocusChart: React.FC<FocusChartProps> = ({ onPriceUpdate }) => {
  const {
    activeStock,
    timeframe,
    forceRefreshCounter,
    setCacheLatency,
    setChartCacheHeader,
    isDrawerOpen,
    toggleDrawer,
    setDrawerTab,
    setLatestCandleInfo,
  } = useApp();

  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | null>(null);

  const [candleData, setCandleData] = useState<Candle[]>([]);
  const [ohlc, setOhlc] = useState({
    open: '--',
    high: '--',
    low: '--',
    close: '--',
    chg: '--',
    vol: '--',
    isPositive: true,
  });

  // Notes and indicator dots state
  const [notes, setNotes] = useState<StockNote[]>([]);
  const [dots, setDots] = useState<{ id: number; title: string; status: string; left: number; timeframe: string }[]>([]);

  // 1. Initialize Chart instance
  useEffect(() => {
    if (!containerRef.current) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: containerRef.current.clientHeight,
      layout: {
        background: { color: '#111726' },
        textColor: '#94a3b8',
        fontSize: 11,
        fontFamily: "'JetBrains Mono', monospace",
      },
      grid: {
        vertLines: { color: '#1a233a', style: 1 },
        horzLines: { color: '#1a233a', style: 1 },
      },
      crosshair: {
        mode: 1,
        vertLine: { color: '#3b82f6', width: 1, style: 3 },
        horzLine: { color: '#3b82f6', width: 1, style: 3 },
      },
      timeScale: {
        borderColor: '#232e47',
        timeVisible: true,
        secondsVisible: false,
      },
      rightPriceScale: {
        borderColor: '#232e47',
        scaleMargins: { top: 0.1, bottom: 0.2 },
      },
    });

    const candleSeries = chart.addCandlestickSeries({
      upColor: '#22c55e',
      downColor: '#ef4444',
      borderVisible: false,
      wickUpColor: '#22c55e',
      wickDownColor: '#ef4444',
    });

    const volumeSeries = chart.addHistogramSeries({
      color: '#3b82f6',
      priceFormat: { type: 'volume' },
      priceScaleId: '', // overlay volume
    });

    volumeSeries.priceScale().applyOptions({
      scaleMargins: { top: 0.8, bottom: 0 },
    });

    chartRef.current = chart;
    candleSeriesRef.current = candleSeries;
    volumeSeriesRef.current = volumeSeries;

    // Crosshair move handler
    chart.subscribeCrosshairMove(param => {
      if (!param || !param.time || !param.seriesData) return;
      const data = param.seriesData.get(candleSeries) as CandlestickData | undefined;
      const volData = param.seriesData.get(volumeSeries) as HistogramData | undefined;

      if (data) {
        const chg = ((data.close - data.open) / data.open) * 100;
        const volStr = volData?.value ? (volData.value > 1e6 ? `${(volData.value / 1e6).toFixed(2)}M` : `${(volData.value / 1e3).toFixed(0)}K`) : '--';

        setOhlc({
          open: `₹${data.open.toFixed(2)}`,
          high: `₹${data.high.toFixed(2)}`,
          low: `₹${data.low.toFixed(2)}`,
          close: `₹${data.close.toFixed(2)}`,
          chg: `${chg >= 0 ? '+' : ''}${chg.toFixed(2)}%`,
          vol: volStr,
          isPositive: chg >= 0,
        });
      }
    });

    // Resize observer
    const ro = new ResizeObserver(entries => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        chart.resize(width, height);
      }
    });
    ro.observe(containerRef.current);

    return () => {
      ro.disconnect();
      chart.remove();
      chartRef.current = null;
      candleSeriesRef.current = null;
      volumeSeriesRef.current = null;
    };
  }, []);

  // 2. Fetch candle data when activeStock, timeframe, or forceRefresh changes
  useEffect(() => {
    if (!activeStock || !candleSeriesRef.current || !volumeSeriesRef.current) return;

    let isSubscribed = true;
    const startTime = performance.now();

    async function loadData() {
      try {
        const { candles, cacheHeader } = await api.getCandles(
          activeStock!.symbol,
          timeframe,
          forceRefreshCounter > 0
        );

        if (!isSubscribed) return;

        const latency = (performance.now() - startTime).toFixed(1);
        setCacheLatency(`${latency}ms (${cacheHeader})`);
        setChartCacheHeader(cacheHeader);
        setCandleData(candles);

        if (candles.length > 0) {
          const formattedCandles: CandlestickData[] = candles.map(c => ({
            time: c.timestamp as Time,
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
          }));

          const formattedVolumes: HistogramData[] = candles.map(c => ({
            time: c.timestamp as Time,
            value: c.volume,
            color: c.close >= c.open ? 'rgba(34, 197, 94, 0.4)' : 'rgba(239, 68, 68, 0.4)',
          }));

          candleSeriesRef.current?.setData(formattedCandles);
          volumeSeriesRef.current?.setData(formattedVolumes);

          // Update latest prices
          const last = candles[candles.length - 1];
          setLatestCandleInfo(last.close, last.timestamp);
          const first = candles[0];
          const totalChg = ((last.close - first.open) / first.open) * 100;
          const isPos = totalChg >= 0;

          if (onPriceUpdate) {
            onPriceUpdate(`₹${last.close.toFixed(2)}`, {
              text: `${isPos ? '+' : ''}${totalChg.toFixed(2)}%`,
              isPositive: isPos,
            });
          }

          chartRef.current?.timeScale().fitContent();
        }
      } catch (err) {
        console.warn('Failed to load candles', err);
      }
    }

    loadData();

    return () => {
      isSubscribed = false;
    };
  }, [activeStock, timeframe, forceRefreshCounter, setCacheLatency, setChartCacheHeader, onPriceUpdate]);

  // 3. Fetch Stock Notes and position x-axis indicator dots
  useEffect(() => {
    if (!activeStock) return;
    let isSubscribed = true;

    async function loadNotes() {
      try {
        const data = await api.getNotes(activeStock!.symbol);
        if (isSubscribed) setNotes(data);
      } catch (_) {
        // ignore
      }
    }

    loadNotes();
    return () => {
      isSubscribed = false;
    };
  }, [activeStock, forceRefreshCounter]);

  // Canonicalize timeframe string for matching
  const canonicalTimeframe = (tf: string) => {
    const s = String(tf || '').toLowerCase();
    return s === '60m' ? '1h' : s === '1wk' ? '1w' : s;
  };

  // Re-position x-axis dots
  const updateDotsPosition = useCallback(() => {
    const chart = chartRef.current;
    if (!chart || candleData.length === 0 || notes.length === 0) {
      setDots([]);
      return;
    }

    const timeScale = chart.timeScale();
    const activeTf = canonicalTimeframe(timeframe);
    const tfNotes = notes.filter(n => canonicalTimeframe(n.timeframe) === activeTf);

    const minTime = candleData[0].timestamp;
    const maxTime = candleData[candleData.length - 1].timestamp;
    const interval = candleData.length > 1 ? Math.max(1, candleData[1].timestamp - candleData[0].timestamp) : 900;

    const visibleNotes = tfNotes.filter(n => {
      const ts = n.candle_timestamp || n.created_at;
      return ts && ts >= minTime - interval && ts <= maxTime + interval;
    });

    const newDots = visibleNotes
      .map(note => {
        const ts = note.candle_timestamp || note.created_at;
        if (!ts) return null;

        // Snap to nearest candle
        let snapTs = candleData[0].timestamp;
        let minDiff = Math.abs(candleData[0].timestamp - ts);
        for (let i = 1; i < candleData.length; i++) {
          const diff = Math.abs(candleData[i].timestamp - ts);
          if (diff < minDiff) {
            minDiff = diff;
            snapTs = candleData[i].timestamp;
          }
        }

        if (minDiff > interval * 3) return null;

        const x = timeScale.timeToCoordinate(snapTs as Time);
        if (x === null || x < 0) return null;

        return {
          id: note.id,
          title: note.title,
          status: note.status,
          timeframe: note.timeframe,
          left: x - 3.5,
        };
      })
      .filter(Boolean) as { id: number; title: string; status: string; left: number; timeframe: string }[];

    setDots(newDots);
  }, [candleData, notes, timeframe]);

  // Subscribe to timeScale changes
  useEffect(() => {
    const chart = chartRef.current;
    if (!chart) return;

    updateDotsPosition();
    chart.timeScale().subscribeVisibleTimeRangeChange(updateDotsPosition);

    return () => {
      chart.timeScale().unsubscribeVisibleTimeRangeChange(updateDotsPosition);
    };
  }, [updateDotsPosition]);

  // Handle dot click: open journal drawer & scroll to note
  const handleDotClick = (noteId: number) => {
    if (!isDrawerOpen) toggleDrawer();
    setDrawerTab('journal');

    setTimeout(() => {
      const el = document.getElementById(`note-card-${noteId}`);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'center' });
        el.style.outline = '2px solid var(--accent)';
        el.style.outlineOffset = '2px';
        setTimeout(() => {
          el.style.outline = '';
          el.style.outlineOffset = '';
        }, 2500);
      }
    }, 350);
  };

  return (
    <div className="chart-box" id="chart-box">
      {/* Floating OHLC Strip */}
      <div className="ohlc-strip" id="ohlc-strip">
        <div>
          O: <span>{ohlc.open}</span>
        </div>
        <div>
          H: <span>{ohlc.high}</span>
        </div>
        <div>
          L: <span>{ohlc.low}</span>
        </div>
        <div>
          C: <span>{ohlc.close}</span>
        </div>
        <div>
          Chg:{' '}
          <span style={{ color: ohlc.isPositive ? 'var(--bullish)' : 'var(--bearish)' }}>
            {ohlc.chg}
          </span>
        </div>
        <div>
          Vol: <span>{ohlc.vol}</span>
        </div>
      </div>

      {/* Main Lightweight Charts Canvas Container */}
      <div ref={containerRef} id="chart-canvas" style={{ width: '100%', height: '100%' }} />

      {/* Note Indicator Strip (Bottom x-axis dots) */}
      <div className="note-indicator-strip" id="note-indicator-strip">
        {dots.map(dot => (
          <div
            key={dot.id}
            className={`note-dot ${dot.status}`}
            style={{ left: `${dot.left}px` }}
            title={`#${dot.id}: ${dot.title} (${dot.timeframe})`}
            onClick={e => {
              e.stopPropagation();
              handleDotClick(dot.id);
            }}
          />
        ))}
      </div>
    </div>
  );
};
