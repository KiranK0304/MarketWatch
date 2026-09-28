import React, { useState, useEffect, useCallback } from 'react';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import type { StockNote } from '../../types';

export const NoteModal: React.FC = () => {
  const {
    isNoteModalOpen,
    editingNote,
    closeNoteModal,
    activeStock,
    timeframe,
    latestPrice,
    latestCandleTimestamp,
    showToast,
    triggerForceRefresh,
  } = useApp();

  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [targetPrice, setTargetPrice] = useState('');
  const [stopLoss, setStopLoss] = useState('');
  const [tags, setTags] = useState('');
  const [referenceIds, setReferenceIds] = useState<number[]>([]);
  const [availableNotes, setAvailableNotes] = useState<StockNote[]>([]);
  const [isSaving, setIsSaving] = useState(false);

  // Side panel expandable width state (persisted in localStorage)
  const [panelWidth, setPanelWidth] = useState<number>(() => {
    const saved = localStorage.getItem('mw_note_panel_width');
    const parsed = saved ? parseInt(saved, 10) : 460;
    return Number.isFinite(parsed) && parsed >= 350 && parsed <= 900 ? parsed : 460;
  });
  const [isExpanded, setIsExpanded] = useState<boolean>(false);

  useEffect(() => {
    localStorage.setItem('mw_note_panel_width', String(panelWidth));
    setIsExpanded(panelWidth >= 650);
  }, [panelWidth]);

  const toggleExpand = useCallback(() => {
    if (isExpanded) {
      setPanelWidth(460);
      setIsExpanded(false);
    } else {
      setPanelWidth(720);
      setIsExpanded(true);
    }
  }, [isExpanded]);

  // Drag resizer for the left edge of the side panel
  const startResizing = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startWidth = panelWidth;

      const onMouseMove = (moveEvent: MouseEvent) => {
        const delta = startX - moveEvent.clientX;
        const newWidth = Math.max(350, Math.min(900, startWidth + delta));
        setPanelWidth(newWidth);
      };

      const onMouseUp = () => {
        window.removeEventListener('mousemove', onMouseMove);
        window.removeEventListener('mouseup', onMouseUp);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      };

      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
      window.addEventListener('mousemove', onMouseMove);
      window.addEventListener('mouseup', onMouseUp);
    },
    [panelWidth]
  );

  // Initialize form when opening or editingNote changes
  useEffect(() => {
    if (!isNoteModalOpen) return;
    let isSubscribed = true;

    if (editingNote) {
      setTitle(editingNote.title);
      setContent(editingNote.content);
      setTargetPrice(editingNote.target_price ? String(editingNote.target_price) : '');
      setStopLoss(editingNote.stop_loss ? String(editingNote.stop_loss) : '');
      setTags(editingNote.tags || '');
      setReferenceIds(editingNote.reference_note_ids || []);
    } else {
      setTitle('');
      setContent('');
      setTargetPrice('');
      setStopLoss('');
      setTags('');
      setReferenceIds([]);
    }

    // Load existing notes for references
    const sym = editingNote ? editingNote.symbol : activeStock?.symbol;
    if (sym) {
      api.getNotes(sym).then(notes => {
        if (!isSubscribed) return;
        // Exclude current note from references
        const filtered = editingNote ? notes.filter(n => n.id !== editingNote.id) : notes;
        setAvailableNotes(filtered);
      });
    }

    return () => {
      isSubscribed = false;
    };
  }, [isNoteModalOpen, editingNote, activeStock]);

  if (!isNoteModalOpen) return null;

  const currentSym = editingNote ? editingNote.symbol : activeStock?.symbol || '--';
  const currentTf = editingNote ? editingNote.timeframe : timeframe;
  const currentPrice = editingNote
    ? `₹${editingNote.price_at_note.toFixed(2)}`
    : latestPrice > 0
    ? `₹${latestPrice.toFixed(2)}`
    : '--';

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim() || !content.trim()) {
      showToast('Title and content are required');
      return;
    }
    if (!editingNote && (!activeStock || latestPrice <= 0 || latestCandleTimestamp === null)) {
      showToast('Wait for the active stock chart to finish loading');
      return;
    }
    // Reject non-numeric prices client-side: parseFloat("abc") is NaN, which
    // JSON.stringify turns into null — silently dropping the user's input.
    const parseOptionalPrice = (raw: string, field: string): number | null | undefined => {
      const t = raw.trim();
      if (!t) return null;
      const v = parseFloat(t);
      if (!Number.isFinite(v) || v <= 0) {
        showToast(`${field} must be a positive number`);
        return undefined;
      }
      return v;
    };
    const target = parseOptionalPrice(targetPrice, 'Target price');
    if (target === undefined) return;
    const stop = parseOptionalPrice(stopLoss, 'Stop loss');
    if (stop === undefined) return;

    setIsSaving(true);
    try {
      if (editingNote) {
        // Update existing note
        await api.updateNote(editingNote.id, {
          title: title.trim(),
          content: content.trim(),
          tags: tags.trim(),
          target_price: target,
          stop_loss: stop,
          reference_note_ids: referenceIds,
        });
        showToast(`Note #${editingNote.id} updated`);
      } else {
        // Create new note
        await api.createNote({
          symbol: currentSym,
          timeframe: currentTf,
          candle_timestamp: latestCandleTimestamp,
          price_at_note: latestPrice,
          title: title.trim(),
          content: content.trim(),
          tags: tags.trim(),
          target_price: target,
          stop_loss: stop,
          reference_note_ids: referenceIds,
        });
        showToast(`New note logged for ${currentSym}`);
      }

      triggerForceRefresh();
      closeNoteModal();
    } catch (err: any) {
      showToast(`Failed to save note: ${err.message}`);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <aside
      className="note-side-panel"
      id="note-side-panel"
      style={{ width: `${panelWidth}px` }}
    >
      {/* Draggable left edge resizer */}
      <div
        className="note-panel-resizer"
        onMouseDown={startResizing}
        title="Drag to resize note panel"
      />

      {/* Header */}
      <div className="note-modal-header">
        <div className="note-modal-title-row">
          <span className="note-modal-icon">📝</span>
          <span className="note-modal-title">
            {editingNote ? 'Edit Analysis Note' : 'Log Analysis Note'}
          </span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <button
            type="button"
            className="top-action-btn"
            style={{ padding: '2px 8px', fontSize: '11px', height: '24px' }}
            title={isExpanded ? 'Compress side panel' : 'Expand side panel'}
            onClick={toggleExpand}
          >
            {isExpanded ? '⤢ Compress' : '⤡ Expand'}
          </button>
          <button
            className="note-modal-close"
            title="Close (Esc)"
            onClick={closeNoteModal}
          >
            &times;
          </button>
        </div>
      </div>

      {/* Context Chips (Symbol, Timeframe, Live Price) */}
      <div className="note-modal-context">
        <div className="note-ctx-chip">
          <span className="note-ctx-label">Symbol</span>
          <span className="note-ctx-value">{currentSym}</span>
        </div>
        <div className="note-ctx-chip">
          <span className="note-ctx-label">Timeframe</span>
          <span className="note-ctx-value">{currentTf}</span>
        </div>
        <div className="note-ctx-chip">
          <span className="note-ctx-label">Price</span>
          <span className="note-ctx-value">{currentPrice}</span>
        </div>
      </div>

      {/* Note Form */}
      <form
        onSubmit={handleSave}
        style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }}
      >
        <div className="note-modal-body" style={{ flex: 1, overflowY: 'auto' }}>
          <div className="note-field">
            <label className="note-field-label">
              Title / Setup Headline <span className="note-required">*</span>
            </label>
            <input
              type="text"
              className="note-field-input"
              placeholder="e.g. Bullish momentum breakout above 200 EMA"
              value={title}
              onChange={e => setTitle(e.target.value)}
              required
            />
          </div>

          <div className="note-field">
            <label className="note-field-label">
              Thesis & Technical Observations <span className="note-required">*</span>
            </label>
            <textarea
              className="note-field-input note-field-textarea"
              rows={isExpanded ? 8 : 5}
              placeholder="Volume surge, RSI divergence, support level holding, candlestick pattern..."
              value={content}
              onChange={e => setContent(e.target.value)}
              required
            />
          </div>

          <div className="note-field-row">
            <div className="note-field">
              <label className="note-field-label">Target Price (₹)</label>
              <input
                type="number"
                step="0.05"
                className="note-field-input"
                placeholder="1720.00"
                value={targetPrice}
                onChange={e => setTargetPrice(e.target.value)}
              />
            </div>
            <div className="note-field">
              <label className="note-field-label">Stop Loss (₹)</label>
              <input
                type="number"
                step="0.05"
                className="note-field-input"
                placeholder="1610.00"
                value={stopLoss}
                onChange={e => setStopLoss(e.target.value)}
              />
            </div>
          </div>

          <div className="note-field">
            <label className="note-field-label">Tags</label>
            <input
              type="text"
              className="note-field-input"
              placeholder="momentum, breakout, resistance"
              value={tags}
              onChange={e => setTags(e.target.value)}
            />
          </div>

          {availableNotes.length > 0 && (
            <div className="note-field">
              <label className="note-field-label">Link Previous Notes</label>
              <select
                multiple
                className="note-field-input note-field-select"
                value={referenceIds.map(String)}
                onChange={e => {
                  const selected = Array.from(e.target.selectedOptions, o => parseInt(o.value, 10));
                  setReferenceIds(selected);
                }}
              >
                {availableNotes.map(n => (
                  <option key={n.id} value={n.id}>
                    #{n.id} ({n.timeframe}): {n.title}
                  </option>
                ))}
              </select>
              <span className="note-field-hint">Hold Ctrl/Cmd to select multiple referenced notes</span>
            </div>
          )}
        </div>

        <div className="note-modal-footer">
          <button type="button" className="note-btn note-btn-cancel" onClick={closeNoteModal}>
            Cancel
          </button>
          <button type="submit" className="note-btn note-btn-save" disabled={isSaving}>
            {isSaving ? 'Saving...' : 'Save Note'}
          </button>
        </div>
      </form>
    </aside>
  );
};
