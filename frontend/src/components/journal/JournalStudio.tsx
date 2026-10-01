import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import { FocusChart } from '../chart/FocusChart';
import type { StockNote, NoteStatus } from '../../types';

interface JournalStudioProps {
  onPriceUpdate?: (price: string, change: { text: string; isPositive: boolean }) => void;
}

export const JournalStudio: React.FC<JournalStudioProps> = ({ onPriceUpdate }) => {
  const {
    activeStock,
    selectStock,
    timeframe,
    latestPrice,
    selectedJournalNote,
    setSelectedJournalNote,
    openAuditModal,
    showToast,
    forceRefreshCounter,
    triggerForceRefresh,
    refreshJournalStocks,
  } = useApp();

  // Local state for notes list and filters
  const [notes, setNotes] = useState<StockNote[]>([]);
  const [loading, setLoading] = useState(false);
  const [scope, setScope] = useState<'stock' | 'all'>('stock');
  const [statusFilter, setStatusFilter] = useState<'all' | 'open' | 'validated' | 'invalidated' | 'cancelled'>('all');
  const [searchQuery, setSearchQuery] = useState('');

  // Inline note creation panel state
  const [isCreating, setIsCreating] = useState(false);
  const [formTitle, setFormTitle] = useState('');
  const [formContent, setFormContent] = useState('');
  const [formTags, setFormTags] = useState('');
  const [formPrice, setFormPrice] = useState<string>('');
  const [formTarget, setFormTarget] = useState<string>('');
  const [formStopLoss, setFormStopLoss] = useState<string>('');
  const [saving, setSaving] = useState(false);

  // Fetch notes whenever stock, scope, status, or refresh counter changes
  const fetchNotes = useCallback(async () => {
    setLoading(true);
    try {
      const sym = scope === 'stock' ? activeStock?.symbol : undefined;
      const statusParam = statusFilter === 'all' ? undefined : (statusFilter as any);
      const data = await api.getNotes(sym, statusParam);
      setNotes(data);
    } catch (err: any) {
      showToast(`Failed to load journal notes: ${err.message}`);
    } finally {
      setLoading(false);
    }
  }, [activeStock?.symbol, scope, statusFilter, showToast]);

  useEffect(() => {
    fetchNotes();
  }, [fetchNotes, forceRefreshCounter]);

  // Pre-fill creation form entry price from latest price
  useEffect(() => {
    if (latestPrice > 0 && !formPrice) {
      setFormPrice(latestPrice.toFixed(2));
    }
  }, [latestPrice, formPrice]);

  // Compute overall performance statistics
  const stats = useMemo(() => {
    const total = notes.length;
    const openCount = notes.filter(n => n.status === 'open').length;
    const validatedCount = notes.filter(n => n.status === 'validated').length;
    const invalidatedCount = notes.filter(n => n.status === 'invalidated').length;
    const closed = validatedCount + invalidatedCount;
    const winRate = closed > 0 ? ((validatedCount / closed) * 100).toFixed(1) : '--';

    // Average Risk-to-Reward ratio for notes with both target and stop loss
    let totalRR = 0;
    let rrCount = 0;
    for (const n of notes) {
      if (n.target_price && n.stop_loss && n.price_at_note > 0) {
        const potentialProfit = Math.abs(n.target_price - n.price_at_note);
        const potentialLoss = Math.abs(n.price_at_note - n.stop_loss);
        if (potentialLoss > 0) {
          totalRR += potentialProfit / potentialLoss;
          rrCount++;
        }
      }
    }
    const avgRR = rrCount > 0 ? (totalRR / rrCount).toFixed(2) : '--';

    return {
      total,
      openCount,
      validatedCount,
      invalidatedCount,
      winRate,
      avgRR,
    };
  }, [notes]);

  // Live Risk/Reward calculation for inline creator
  const liveRR = useMemo(() => {
    const entry = parseFloat(formPrice);
    const target = parseFloat(formTarget);
    const sl = parseFloat(formStopLoss);

    if (entry > 0 && target > 0 && sl > 0) {
      const reward = Math.abs(target - entry);
      const risk = Math.abs(entry - sl);
      const rewardPct = ((target - entry) / entry) * 100;
      const riskPct = ((sl - entry) / entry) * 100;
      const ratio = risk > 0 ? (reward / risk).toFixed(2) : '--';

      return {
        ratio,
        rewardPct: rewardPct.toFixed(2),
        riskPct: riskPct.toFixed(2),
        isBullish: target > entry,
      };
    }
    return null;
  }, [formPrice, formTarget, formStopLoss]);

  // Filter notes by search query
  const filteredNotes = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return notes;
    return notes.filter(
      n =>
        n.title.toLowerCase().includes(q) ||
        n.content.toLowerCase().includes(q) ||
        n.symbol.toLowerCase().includes(q) ||
        (n.tags && n.tags.toLowerCase().includes(q))
    );
  }, [notes, searchQuery]);

  // Save new trade setup
  const handleSaveSetup = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formTitle.trim() || !formContent.trim()) {
      showToast('Please provide a title and thesis for your setup.');
      return;
    }
    if (!activeStock) {
      showToast('No active stock selected.');
      return;
    }

    setSaving(true);
    try {
      const entryPrice = parseFloat(formPrice) || latestPrice || 0;
      const targetPrice = formTarget ? parseFloat(formTarget) : undefined;
      const stopLoss = formStopLoss ? parseFloat(formStopLoss) : undefined;

      const created = await api.createNote({
        symbol: activeStock.symbol,
        timeframe,
        price_at_note: entryPrice,
        title: formTitle.trim(),
        content: formContent.trim(),
        tags: formTags.trim() || undefined,
        target_price: targetPrice,
        stop_loss: stopLoss,
      });

      showToast(`Trade setup #${created.id} saved for ${activeStock.symbol}!`);
      setFormTitle('');
      setFormContent('');
      setFormTags('');
      setFormTarget('');
      setFormStopLoss('');
      setIsCreating(false);
      triggerForceRefresh();
      await refreshJournalStocks();
      await fetchNotes();
      setSelectedJournalNote(created);
    } catch (err: any) {
      showToast(`Failed to save setup: ${err.message}`);
    } finally {
      setSaving(false);
    }
  };

  // Quick update note status inline
  const handleQuickStatusChange = async (note: StockNote, newStatus: NoteStatus) => {
    try {
      const updated = await api.updateNote(note.id, {
        status: newStatus,
        verified_at: newStatus !== 'open' ? Math.floor(Date.now() / 1000) : null,
      });
      showToast(`Note #${note.id} updated to ${newStatus}`);
      triggerForceRefresh();
      await fetchNotes();
      if (selectedJournalNote?.id === note.id) {
        setSelectedJournalNote(updated);
      }
    } catch (err: any) {
      showToast(`Failed to update status: ${err.message}`);
    }
  };

  // Delete note
  const handleDeleteNote = async (id: number) => {
    if (!window.confirm(`Delete Trade Note #${id}?`)) return;
    try {
      await api.deleteNote(id);
      showToast(`Note #${id} deleted`);
      if (selectedJournalNote?.id === id) {
        setSelectedJournalNote(null);
      }
      triggerForceRefresh();
      await refreshJournalStocks();
      await fetchNotes();
    } catch (err: any) {
      showToast(`Failed to delete note: ${err.message}`);
    }
  };

  // Clickable suggested tags
  const suggestedTags = ['#breakout', '#pullback', '#retest', '#momentum', '#support-bounce', '#reversal'];

  return (
    <div className="journal-studio-container">
      {/* 1. Left Section: Chart Stage with Live Target/SL Projection Lines */}
      <section className="journal-studio-chart-pane">
        {selectedJournalNote && (
          <div className="journal-studio-active-banner">
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
              <span className="journal-studio-pulse" />
              <span style={{ fontWeight: 700, fontSize: '11px', color: 'var(--text-primary)' }}>
                Viewing Setup #{selectedJournalNote.id}:
              </span>
              <span style={{ fontSize: '11px', color: 'var(--text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {selectedJournalNote.title}
              </span>
            </div>
            <div style={{ display: 'flex', gap: '10px', fontSize: '11px', flexShrink: 0 }}>
              {selectedJournalNote.price_at_note > 0 && (
                <span style={{ color: '#38bdf8' }}>📍 Entry: ₹{selectedJournalNote.price_at_note.toFixed(2)}</span>
              )}
              {selectedJournalNote.target_price && (
                <span style={{ color: 'var(--bullish)' }}>🎯 Target: ₹{selectedJournalNote.target_price.toFixed(2)}</span>
              )}
              {selectedJournalNote.stop_loss && (
                <span style={{ color: 'var(--bearish)' }}>🛑 SL: ₹{selectedJournalNote.stop_loss.toFixed(2)}</span>
              )}
              <button
                className="top-action-btn"
                style={{ fontSize: '10px', padding: '1px 6px' }}
                onClick={() => setSelectedJournalNote(null)}
                title="Clear chart projection lines"
              >
                ✕ Clear
              </button>
            </div>
          </div>
        )}

        <div className="journal-studio-chart-wrapper">
          <FocusChart onPriceUpdate={onPriceUpdate} />
        </div>
      </section>

      {/* 2. Right Section: The Elaborated Trade Journal Studio Desk */}
      <section className="journal-studio-desk-pane">
        {/* Header & Performance Metrics */}
        <div className="journal-studio-header">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span style={{ fontSize: '18px' }}>📖</span>
              <div>
                <h2 style={{ fontSize: '15px', fontWeight: 800, margin: 0, color: 'var(--text-primary)' }}>
                  Trade Journal Studio
                </h2>
                <div style={{ fontSize: '11px', color: 'var(--text-muted)' }}>
                  Capture chaotic theses $\to$ Quant-audit post-market outcomes
                </div>
              </div>
            </div>

            <button
              className="top-action-btn active"
              style={{ fontSize: '11px', padding: '5px 12px', fontWeight: 700 }}
              onClick={() => setIsCreating(prev => !prev)}
            >
              {isCreating ? '✕ Close Form' : '+ Log Trade Setup'}
            </button>
          </div>

          {/* Quant Performance Scorecard */}
          <div className="journal-stats-grid">
            <div className="journal-stat-card">
              <span className="stat-label">Win Rate</span>
              <span className="stat-val" style={{ color: stats.winRate !== '--' && parseFloat(stats.winRate) >= 50 ? 'var(--bullish)' : 'var(--text-primary)' }}>
                {stats.winRate !== '--' ? `${stats.winRate}%` : '--'}
              </span>
            </div>
            <div className="journal-stat-card">
              <span className="stat-label">Setups Logged</span>
              <span className="stat-val">{stats.total}</span>
            </div>
            <div className="journal-stat-card">
              <span className="stat-label">Active Theses</span>
              <span className="stat-val" style={{ color: '#eab308' }}>
                {stats.openCount} ⏳
              </span>
            </div>
            <div className="journal-stat-card">
              <span className="stat-label">Avg R:R Ratio</span>
              <span className="stat-val" style={{ color: '#38bdf8' }}>
                {stats.avgRR !== '--' ? `${stats.avgRR} : 1` : '--'}
              </span>
            </div>
          </div>
        </div>

        {/* Inline Trade Setup Creator (Expandable Desk Form) */}
        {isCreating && (
          <form className="journal-inline-creator" onSubmit={handleSaveSetup}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
              <span style={{ fontWeight: 700, fontSize: '13px', color: 'var(--accent)' }}>
                🎯 New Trade Setup — {activeStock?.symbol} ({timeframe})
              </span>
              <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>
                Real-time Risk/Reward Calculator
              </span>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <input
                type="text"
                placeholder="Setup Title (e.g., Bullish Flag Breakout above 20 EMA)..."
                value={formTitle}
                onChange={e => setFormTitle(e.target.value)}
                className="journal-input"
                required
              />

              {/* Price, Target, and Stop Loss Row */}
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '8px' }}>
                <div>
                  <label className="journal-field-label">📍 Entry Price</label>
                  <input
                    type="number"
                    step="0.05"
                    placeholder="Entry"
                    value={formPrice}
                    onChange={e => setFormPrice(e.target.value)}
                    className="journal-input"
                  />
                </div>
                <div>
                  <label className="journal-field-label" style={{ color: 'var(--bullish)' }}>🎯 Target Price</label>
                  <input
                    type="number"
                    step="0.05"
                    placeholder="Target"
                    value={formTarget}
                    onChange={e => setFormTarget(e.target.value)}
                    className="journal-input"
                  />
                </div>
                <div>
                  <label className="journal-field-label" style={{ color: 'var(--bearish)' }}>🛑 Stop Loss</label>
                  <input
                    type="number"
                    step="0.05"
                    placeholder="Stop Loss"
                    value={formStopLoss}
                    onChange={e => setFormStopLoss(e.target.value)}
                    className="journal-input"
                  />
                </div>
              </div>

              {/* Live Risk/Reward Feedback Bar */}
              {liveRR && (
                <div className="journal-rr-preview">
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontWeight: 700, fontSize: '11px' }}>
                    <span style={{ color: 'var(--accent)' }}>Planned R:R Ratio: {liveRR.ratio} : 1</span>
                    <span style={{ color: 'var(--bullish)' }}>Target: {liveRR.rewardPct}%</span>
                    <span style={{ color: 'var(--bearish)' }}>Risk: {liveRR.riskPct}%</span>
                  </div>
                </div>
              )}

              {/* Raw Thesis Textarea */}
              <div>
                <label className="journal-field-label">🧑 Trader's Raw Thesis / Chaotic Notes</label>
                <textarea
                  rows={3}
                  placeholder="Stream of thought: Why this trade? Volume conviction? Key moving averages? Where are you wrong?"
                  value={formContent}
                  onChange={e => setFormContent(e.target.value)}
                  className="journal-textarea"
                  required
                />
              </div>

              {/* Tags & Quick Suggestions */}
              <div>
                <label className="journal-field-label">🏷️ Tags (comma separated)</label>
                <input
                  type="text"
                  placeholder="e.g. breakout, high-volume, 20ema"
                  value={formTags}
                  onChange={e => setFormTags(e.target.value)}
                  className="journal-input"
                />
                <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '4px' }}>
                  {suggestedTags.map(tag => (
                    <button
                      key={tag}
                      type="button"
                      className="journal-tag-chip"
                      onClick={() => {
                        const raw = tag.replace('#', '');
                        setFormTags(prev => (prev ? `${prev}, ${raw}` : raw));
                      }}
                    >
                      {tag}
                    </button>
                  ))}
                </div>
              </div>

              {/* Action Buttons */}
              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '4px' }}>
                <button
                  type="button"
                  className="top-action-btn"
                  onClick={() => setIsCreating(false)}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="top-action-btn active"
                  style={{ background: 'var(--accent)', color: 'white' }}
                >
                  {saving ? 'Saving...' : '💾 Save Trade Setup'}
                </button>
              </div>
            </div>
          </form>
        )}

        {/* Filter and Search Bar */}
        <div className="journal-toolbar">
          {/* Scope Toggle: Active Stock vs All */}
          <div className="journal-scope-toggle">
            <button
              className={`scope-pill ${scope === 'stock' ? 'active' : ''}`}
              onClick={() => setScope('stock')}
            >
              {activeStock?.symbol || 'Active Stock'}
            </button>
            <button
              className={`scope-pill ${scope === 'all' ? 'active' : ''}`}
              onClick={() => setScope('all')}
            >
              All Universe ({notes.length})
            </button>
          </div>

          {/* Status Filter Pills */}
          <div className="journal-status-filters">
            {(['all', 'open', 'validated', 'invalidated', 'cancelled'] as const).map(st => (
              <button
                key={st}
                className={`status-pill-btn ${statusFilter === st ? 'active' : ''}`}
                onClick={() => setStatusFilter(st)}
              >
                {st === 'open'
                  ? 'Open ⏳'
                  : st === 'validated'
                  ? 'Validated ✅'
                  : st === 'invalidated'
                  ? 'Invalidated ❌'
                  : st === 'cancelled'
                  ? 'Cancelled ⚪'
                  : 'All'}
              </button>
            ))}
          </div>

          {/* Search Input */}
          <div className="journal-search-wrap">
            <input
              type="text"
              placeholder="Search setups, tags (#breakout)..."
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              className="journal-search-input"
            />
            {searchQuery && (
              <button
                className="journal-search-clear"
                onClick={() => setSearchQuery('')}
              >
                &times;
              </button>
            )}
          </div>
        </div>

        {/* Trade Setups List */}
        <div className="journal-notes-stream">
          {loading ? (
            <div style={{ textAlign: 'center', padding: '40px 0', color: 'var(--text-muted)' }}>
              Loading trade journal...
            </div>
          ) : filteredNotes.length === 0 ? (
            <div className="journal-empty-desk">
              <div style={{ fontSize: '32px', marginBottom: '8px' }}>📝</div>
              <div style={{ fontWeight: 700, fontSize: '14px', marginBottom: '4px' }}>
                No Trade Setups Found
              </div>
              <div style={{ fontSize: '12px', color: 'var(--text-muted)', maxWidth: '340px' }}>
                {searchQuery
                  ? `No entries match "${searchQuery}".`
                  : 'Click "+ Log Trade Setup" to document your technical thesis, planned entry, and stop-loss levels.'}
              </div>
            </div>
          ) : (
            filteredNotes.map(note => {
              const isSelected = selectedJournalNote?.id === note.id;
              const dateStr = new Date(note.created_at * 1000).toLocaleString('en-IN', {
                month: 'short',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
              });

              const tags = note.tags
                ? note.tags.split(',').map(t => t.trim()).filter(Boolean)
                : [];

              // Calculated R:R for note card
              let rrDisplay = '--';
              let rewardPctStr = '';
              let riskPctStr = '';
              if (note.target_price && note.stop_loss && note.price_at_note > 0) {
                const reward = Math.abs(note.target_price - note.price_at_note);
                const risk = Math.abs(note.price_at_note - note.stop_loss);
                if (risk > 0) {
                  rrDisplay = `${(reward / risk).toFixed(2)} : 1`;
                  rewardPctStr = `${((note.target_price - note.price_at_note) / note.price_at_note * 100).toFixed(1)}%`;
                  riskPctStr = `${((note.stop_loss - note.price_at_note) / note.price_at_note * 100).toFixed(1)}%`;
                }
              }

              return (
                <div
                  key={note.id}
                  className={`journal-trade-card ${isSelected ? 'selected' : ''}`}
                  onClick={() => setSelectedJournalNote(note)}
                >
                  {/* Card Header: Symbol, Timeframe, Date, Status */}
                  <div className="trade-card-header">
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <button
                        className="trade-card-sym"
                        onClick={e => {
                          e.stopPropagation();
                          if (note.symbol !== activeStock?.symbol) {
                            selectStock(note.symbol);
                          }
                          setSelectedJournalNote(note);
                        }}
                        title={`Focus chart on ${note.symbol}`}
                      >
                        {note.symbol}
                      </button>
                      <span className="trade-card-tf">{note.timeframe}</span>
                      <span className={`journal-status-pill ${note.status}`}>
                        {note.status}
                      </span>
                    </div>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                      <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>
                        {dateStr}
                      </span>
                    </div>
                  </div>

                  {/* Title */}
                  <div className="trade-card-title">
                    #{note.id}: {note.title}
                  </div>

                  {/* Dual-Layer Layout: Trader's Thesis + AI Quant Synthesis */}
                  <div className="trade-card-dual-body">
                    {/* Layer 1: Trader's Raw Notes */}
                    <div className="trade-card-layer-trader">
                      <div className="layer-badge">🧑 Trader's Raw Capture</div>
                      <div className="trade-card-content">{note.content}</div>

                      {tags.length > 0 && (
                        <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '6px' }}>
                          {tags.map(t => (
                            <span key={t} className="trade-tag-pill">
                              #{t}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>

                    {/* Layer 2: AI Co-Pilot / Quant Synthesis */}
                    <div className="trade-card-layer-quant">
                      <div className="layer-badge quant">🤖 Quant Agent View</div>
                      <div className="quant-metrics-grid">
                        <div className="quant-metric-item">
                          <span className="m-label">Entry</span>
                          <span className="m-val" style={{ color: '#38bdf8' }}>
                            ₹{note.price_at_note.toFixed(2)}
                          </span>
                        </div>
                        <div className="quant-metric-item">
                          <span className="m-label">Target</span>
                          <span className="m-val" style={{ color: 'var(--bullish)' }}>
                            {note.target_price ? `₹${note.target_price.toFixed(2)} (${rewardPctStr})` : '--'}
                          </span>
                        </div>
                        <div className="quant-metric-item">
                          <span className="m-label">Stop Loss</span>
                          <span className="m-val" style={{ color: 'var(--bearish)' }}>
                            {note.stop_loss ? `₹${note.stop_loss.toFixed(2)} (${riskPctStr})` : '--'}
                          </span>
                        </div>
                        <div className="quant-metric-item">
                          <span className="m-label">Risk:Reward</span>
                          <span className="m-val" style={{ fontWeight: 800, color: 'var(--accent)' }}>
                            {rrDisplay}
                          </span>
                        </div>
                      </div>

                      {/* Post-Mortem / Outcome Review Banner */}
                      {note.outcome_note && (
                        <div className="quant-outcome-banner">
                          <span style={{ fontWeight: 700 }}>Post-Trade Outcome: </span>
                          <span>{note.outcome_note}</span>
                        </div>
                      )}
                    </div>
                  </div>

                  {/* Card Bottom Actions */}
                  <div className="trade-card-actions" onClick={e => e.stopPropagation()}>
                    <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                      <button
                        className="top-action-btn"
                        style={{ fontSize: '10px', padding: '2px 7px', background: isSelected ? 'var(--accent)' : 'transparent', color: isSelected ? 'white' : 'inherit' }}
                        onClick={() => {
                          if (note.symbol !== activeStock?.symbol) {
                            selectStock(note.symbol);
                          }
                          setSelectedJournalNote(note);
                        }}
                      >
                        🎯 {isSelected ? 'Focused on Chart' : 'Show on Chart'}
                      </button>

                      {/* Quick Status Toggle */}
                      {note.status === 'open' ? (
                        <>
                          <button
                            className="top-action-btn"
                            style={{ fontSize: '10px', padding: '2px 6px', color: 'var(--bullish)' }}
                            onClick={() => handleQuickStatusChange(note, 'validated')}
                            title="Mark trade setup as Validated (Target Hit)"
                          >
                            ✅ Validate
                          </button>
                          <button
                            className="top-action-btn"
                            style={{ fontSize: '10px', padding: '2px 6px', color: 'var(--bearish)' }}
                            onClick={() => handleQuickStatusChange(note, 'invalidated')}
                            title="Mark trade setup as Invalidated (Stopped Out)"
                          >
                            ❌ Invalidate
                          </button>
                        </>
                      ) : (
                        <button
                          className="top-action-btn"
                          style={{ fontSize: '10px', padding: '2px 6px' }}
                          onClick={() => handleQuickStatusChange(note, 'open')}
                          title="Re-open trade thesis"
                        >
                          ⏳ Re-open
                        </button>
                      )}

                      <button
                        className="top-action-btn"
                        style={{ fontSize: '10px', padding: '2px 6px' }}
                        onClick={() => openAuditModal(note)}
                        title="Edit outcome notes and audit details"
                      >
                        ⚖️ Audit
                      </button>
                    </div>

                    <button
                      className="top-action-btn"
                      style={{ fontSize: '10px', padding: '2px 6px', color: 'var(--bearish)' }}
                      onClick={() => handleDeleteNote(note.id)}
                      title="Delete this trade note"
                    >
                      🗑️
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </section>
    </div>
  );
};
