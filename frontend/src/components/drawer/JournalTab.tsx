import React, { useEffect, useState, useCallback } from 'react';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import type { StockNote } from '../../types';

export const JournalTab: React.FC = () => {
  const {
    activeStock,
    selectStock,
    journalFilterScope,
    setJournalFilterScope,
    journalFilterStatus,
    setJournalFilterStatus,
    openNoteModal,
    openAuditModal,
    showToast,
    forceRefreshCounter,
  } = useApp();

  const [notes, setNotes] = useState<StockNote[]>([]);
  const [loading, setLoading] = useState(false);

  const fetchNotes = useCallback(async () => {
    setLoading(true);
    try {
      const sym = journalFilterScope === 'stock' ? activeStock?.symbol : undefined;
      const data = await api.getNotes(sym, journalFilterStatus);
      setNotes(data);
    } catch (err: any) {
      showToast(`Failed to load journal: ${err.message}`);
    } finally {
      setLoading(false);
    }
  }, [activeStock?.symbol, journalFilterScope, journalFilterStatus, showToast]);

  useEffect(() => {
    fetchNotes();
  }, [fetchNotes, forceRefreshCounter]);

  const handleDelete = async (id: number) => {
    if (!window.confirm(`Are you sure you want to delete Note #${id}?`)) return;
    try {
      await api.deleteNote(id);
      showToast(`Note #${id} deleted`);
      fetchNotes();
    } catch (err: any) {
      showToast(`Failed to delete note: ${err.message}`);
    }
  };

  const jumpToNote = async (refId: number) => {
    try {
      const refNote = await api.getNote(refId);
      if (refNote.symbol !== activeStock?.symbol && journalFilterScope === 'stock') {
        selectStock(refNote.symbol);
      }
      setTimeout(() => {
        const card = document.getElementById(`note-card-${refId}`);
        if (card) {
          card.scrollIntoView({ behavior: 'smooth', block: 'center' });
          card.style.outline = '2px solid var(--accent)';
          card.style.outlineOffset = '2px';
          setTimeout(() => {
            card.style.outline = '';
            card.style.outlineOffset = '';
          }, 2500);
        }
      }, 350);
    } catch (err: any) {
      showToast(`Could not navigate to referenced note: ${err.message}`);
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
      {/* Header & Controls */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ fontWeight: 700, fontSize: '13px' }}>📝 Trading Journal</div>
        <button
          className="top-action-btn active"
          id="btn-journal-new"
          style={{ fontSize: '11px', padding: '3px 8px' }}
          onClick={() => openNoteModal()}
        >
          + New Note
        </button>
      </div>

      {/* Scope Switcher: Active Stock vs All Universe */}
      <div style={{ display: 'flex', gap: '4px', background: 'var(--bg-surface)', padding: '3px', borderRadius: '6px', border: '1px solid var(--border)' }}>
        <button
          className={`journal-scope-btn ${journalFilterScope === 'stock' ? 'active' : ''}`}
          style={{
            flex: 1,
            padding: '4px',
            fontSize: '10px',
            fontWeight: 600,
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer',
            background: journalFilterScope === 'stock' ? 'var(--accent)' : 'transparent',
            color: journalFilterScope === 'stock' ? 'white' : 'var(--text-muted)',
          }}
          onClick={() => setJournalFilterScope('stock')}
        >
          {activeStock?.symbol || 'Active Stock'}
        </button>
        <button
          className={`journal-scope-btn ${journalFilterScope === 'all' ? 'active' : ''}`}
          style={{
            flex: 1,
            padding: '4px',
            fontSize: '10px',
            fontWeight: 600,
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer',
            background: journalFilterScope === 'all' ? 'var(--accent)' : 'transparent',
            color: journalFilterScope === 'all' ? 'white' : 'var(--text-muted)',
          }}
          onClick={() => setJournalFilterScope('all')}
        >
          All Universe
        </button>
      </div>

      {/* Status Filter Pills */}
      <div style={{ display: 'flex', gap: '4px', overflowX: 'auto', paddingBottom: '2px' }}>
        {(['all', 'open', 'validated', 'invalidated'] as const).map(st => (
          <button
            key={st}
            className={`filter-pill ${journalFilterStatus === st ? 'active' : ''}`}
            style={{ fontSize: '10px', padding: '2px 7px', textTransform: 'capitalize' }}
            onClick={() => setJournalFilterStatus(st)}
          >
            {st === 'open' ? 'Open ⏳' : st === 'validated' ? 'Validated ✅' : st === 'invalidated' ? 'Invalidated ❌' : 'All'}
          </button>
        ))}
      </div>

      {/* Notes List */}
      <div id="journal-notes-list" style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '6px' }}>
        {loading ? (
          <div style={{ textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px', padding: '20px 0' }}>
            Loading notes...
          </div>
        ) : notes.length === 0 ? (
          <div style={{ textAlign: 'center', color: 'var(--text-muted)', fontSize: '12px', padding: '30px 10px', background: 'var(--bg-surface)', border: '1px dashed var(--border)', borderRadius: '8px' }}>
            <div style={{ fontSize: '24px', marginBottom: '6px' }}>📝</div>
            <div style={{ fontWeight: 600, marginBottom: '4px' }}>No Analysis Notes Found</div>
            <div style={{ fontSize: '11px' }}>Press <kbd>n</kbd> or click <b>+ New Note</b> to log an observation, momentum setup, or price thesis.</div>
          </div>
        ) : (
          notes.map(note => {
            const dateStr = new Date(note.created_at * 1000).toLocaleString('en-IN', {
              month: 'short',
              day: 'numeric',
              hour: '2-digit',
              minute: '2-digit',
            });

            const tags = note.tags
              ? note.tags.split(',').map(t => t.trim()).filter(Boolean)
              : [];

            const isVal = note.status === 'validated';
            const isInv = note.status === 'invalidated';
            const borderColor = isVal ? 'var(--bullish)' : isInv ? 'var(--bearish)' : 'var(--accent)';
            const statusLabel = isVal ? 'Validated ✅' : isInv ? 'Invalidated ❌' : 'Outcome Review';

            return (
              <div key={note.id} className="journal-card" id={`note-card-${note.id}`}>
                {/* Card Top: Symbol, Timeframe, Status, Date */}
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: '6px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap' }}>
                    <a
                      className="journal-stock-link"
                      style={{ fontWeight: 700, fontSize: '12px', color: 'var(--accent)', cursor: 'pointer', textDecoration: 'none' }}
                      onClick={() => selectStock(note.symbol)}
                    >
                      {note.symbol}
                    </a>
                    <span style={{ fontSize: '10px', color: 'var(--text-muted)', background: 'var(--bg-hover)', padding: '1px 4px', borderRadius: '3px' }}>
                      {note.timeframe}
                    </span>
                    <span className={`journal-status-pill ${note.status}`}>
                      {note.status}
                    </span>
                  </div>
                  <span style={{ fontSize: '10px', color: 'var(--text-muted)', whiteSpace: 'nowrap' }}>
                    {dateStr}
                  </span>
                </div>

                {/* Title */}
                <div style={{ fontWeight: 700, fontSize: '13px', color: 'var(--text-primary)', lineHeight: 1.3 }}>
                  #{note.id}: {note.title}
                </div>

                {/* Price at note */}
                <div style={{ fontSize: '11px', color: 'var(--text-muted)' }}>
                  Price at note: <strong style={{ color: 'var(--text-primary)' }}>₹{note.price_at_note.toFixed(2)}</strong>
                </div>

                {/* Content / Thesis */}
                <div style={{ fontSize: '12px', color: 'var(--text-secondary)', lineHeight: 1.4, whiteSpace: 'pre-wrap' }}>
                  {note.content}
                </div>

                {/* Target & Stop Loss */}
                {(note.target_price || note.stop_loss) && (
                  <div style={{ display: 'flex', gap: '8px', fontSize: '11px', marginTop: '2px' }}>
                    {note.target_price && <span style={{ color: 'var(--bullish)' }}>🎯 Target: ₹{note.target_price.toFixed(2)}</span>}
                    {note.stop_loss && <span style={{ color: 'var(--bearish)' }}>🛑 SL: ₹{note.stop_loss.toFixed(2)}</span>}
                  </div>
                )}

                {/* Tags */}
                {tags.length > 0 && (
                  <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '2px' }}>
                    {tags.map(t => (
                      <span key={t} style={{ fontSize: '10px', background: 'var(--bg-hover)', color: 'var(--text-muted)', padding: '1px 5px', borderRadius: '3px' }}>
                        #{t}
                      </span>
                    ))}
                  </div>
                )}

                {/* References */}
                {note.reference_note_ids && note.reference_note_ids.length > 0 && (
                  <div style={{ display: 'flex', gap: '4px', alignItems: 'center', flexWrap: 'wrap', marginTop: '4px' }}>
                    <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>Refs:</span>
                    {note.reference_note_ids.map(rid => (
                      <a
                        key={rid}
                        className="journal-ref-link"
                        title={`Jump to Note #${rid}`}
                        onClick={() => jumpToNote(rid)}
                      >
                        ↳ Ref #{rid}
                      </a>
                    ))}
                  </div>
                )}

                {/* Outcome Review Box */}
                {(note.status !== 'open' || note.outcome_note) && (
                  <div style={{ background: 'var(--bg-subtle)', borderLeft: `3px solid ${borderColor}`, padding: '6px 8px', borderRadius: '4px', fontSize: '11px' }}>
                    <div style={{ fontWeight: 700, color: borderColor, marginBottom: '2px' }}>{statusLabel}</div>
                    <div style={{ color: 'var(--text-secondary)', lineHeight: 1.4 }}>{note.outcome_note || 'Status updated.'}</div>
                  </div>
                )}

                {/* Actions: Verify / Edit / Delete */}
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderTop: '1px solid var(--border-subtle)', paddingTop: '6px', marginTop: '4px' }}>
                  <button
                    className="top-action-btn"
                    style={{ fontSize: '10px', padding: '2px 6px' }}
                    onClick={() => openAuditModal(note)}
                  >
                    {note.status === 'open' ? '⚖️ Verify Outcome' : '✏️ Edit Outcome'}
                  </button>
                  <div style={{ display: 'flex', gap: '4px' }}>
                    <button
                      className="top-action-btn"
                      style={{ fontSize: '10px', padding: '2px 6px' }}
                      onClick={() => openNoteModal(note)}
                    >
                      Edit
                    </button>
                    <button
                      className="top-action-btn"
                      style={{ fontSize: '10px', padding: '2px 6px', color: 'var(--bearish)' }}
                      onClick={() => handleDelete(note.id)}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
