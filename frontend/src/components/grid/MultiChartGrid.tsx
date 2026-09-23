import React, { useEffect, useRef, useState } from 'react';
import { createChart, IChartApi, CandlestickData, Time } from 'lightweight-charts';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import type { Stock } from '../../types';

interface GridCardProps {
  stock: Stock;
  timeframe: string;
  onSelect: (symbol: string) => void;
}

const GridCard: React.FC<GridCardProps> = ({ stock, timeframe, onSelect }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const [priceInfo, setPriceInfo] = useState<{ price: string; chg: string; isPos: boolean }>({
    price: '₹--',
    chg: '--',
    isPos: true,
  });

  useEffect(() => {
    if (!containerRef.current) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: 120,
      layout: {
        background: { color: 'transparent' },
        textColor: '#64748b',
        fontSize: 10,
      },
      grid: {
        vertLines: { visible: false },
        horzLines: { visible: false },
      },
      timeScale: { visible: false },
      rightPriceScale: { visible: false },
      crosshair: { vertLine: { visible: false }, horzLine: { visible: false } },
      handleScroll: false,
      handleScale: false,
    });

    const series = chart.addCandlestickSeries({
      upColor: '#22c55e',
      downColor: '#ef4444',
      borderVisible: false,
      wickUpColor: '#22c55e',
      wickDownColor: '#ef4444',
    });

    chartRef.current = chart;

    let isSubscribed = true;
    async function loadMiniCandles() {
      try {
        const { candles } = await api.getCandles(stock.symbol, timeframe as any);
        if (!isSubscribed || candles.length === 0) return;

        const formatted: CandlestickData[] = candles.slice(-50).map(c => ({
          time: c.timestamp as Time,
          open: c.open,
          high: c.high,
          low: c.low,
          close: c.close,
        }));

        series.setData(formatted);
        chart.priceScale('right').applyOptions({
          autoScale: true,
          scaleMargins: { top: 0.05, bottom: 0.15 },
        });
        chart.timeScale().fitContent();

        const last = candles[candles.length - 1];
        const first = candles[0];
        const chg = first.open > 0 ? ((last.close - first.open) / first.open) * 100 : 0;
        const isPos = chg >= 0;

        setPriceInfo({
          price: `₹${last.close.toFixed(2)}`,
          chg: `${isPos ? '+' : ''}${chg.toFixed(2)}%`,
          isPos,
        });
      } catch (_) {
        // ignore
      }
    }

    loadMiniCandles();

    const ro = new ResizeObserver(entries => {
      for (const entry of entries) {
        chart.resize(entry.contentRect.width, 120);
      }
    });
    ro.observe(containerRef.current);

    return () => {
      isSubscribed = false;
      ro.disconnect();
      chart.remove();
      chartRef.current = null;
    };
  }, [stock.symbol, timeframe]);

  return (
    <div className="grid-chart-card" onClick={() => onSelect(stock.symbol)}>
      <div className="grid-chart-header">
        <div>
          <div className="grid-chart-sym">{stock.symbol}</div>
          <div className="grid-chart-name">{stock.name}</div>
        </div>
        <div style={{ textAlign: 'right' }}>
          <div className="grid-chart-price">{priceInfo.price}</div>
          <div className={`grid-chart-chg ${priceInfo.isPos ? 'up' : 'down'}`}>
            {priceInfo.chg}
          </div>
        </div>
      </div>
      <div ref={containerRef} className="grid-chart-canvas" style={{ height: '120px', width: '100%' }} />
    </div>
  );
};

export const MultiChartGrid: React.FC = () => {
  const {
    stocks,
    filteredMovers,
    sidebarTab,
    sidebarSearch,
    gridPage,
    gridPageSize,
    setGridPage,
    timeframe,
    selectStock,
    setViewMode,
  } = useApp();

  // Filter items based on active sidebar tab (Universe vs Movers) and sidebarSearch query
  const items =
    sidebarTab === 'universe'
      ? stocks
      : filteredMovers.map(m => ({ symbol: m.symbol, name: m.name || m.symbol }));

  const filteredItems = items.filter(s => {
    if (!sidebarSearch.trim()) return true;
    const q = sidebarSearch.toLowerCase();
    return s.symbol.toLowerCase().includes(q) || s.name.toLowerCase().includes(q);
  });

  // Paginate
  const startIndex = (gridPage - 1) * gridPageSize;
  const pageItems = filteredItems.slice(startIndex, startIndex + gridPageSize);
  const totalPages = Math.max(1, Math.ceil(filteredItems.length / gridPageSize));

  useEffect(() => {
    if (gridPage > totalPages) {
      setGridPage(totalPages);
    }
  }, [gridPage, totalPages, setGridPage]);

  const handleSelect = (symbol: string) => {
    selectStock(symbol);
    setViewMode('single');
  };

  return (
    <div id="grid-box" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(240px, 1fr))', gap: '12px', padding: '16px', overflowY: 'auto', flex: 1 }}>
      {pageItems.length === 0 ? (
        <div style={{ gridColumn: '1 / -1', padding: '60px', textAlign: 'center', color: 'var(--text-muted)' }}>
          No stocks found matching grid filter.
        </div>
      ) : (
        pageItems.map(stock => (
          <GridCard
            key={stock.symbol}
            stock={stock}
            timeframe={timeframe}
            onSelect={handleSelect}
          />
        ))
      )}
    </div>
  );
};
