import React, { useState, useEffect } from 'react';
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

    setIsSaving(true);
    try {
      if (editingNote) {
        // Update existing note
        await api.updateNote(editingNote.id, {
          title: title.trim(),
          content: content.trim(),
          tags: tags.trim(),
          target_price: targetPrice.trim() ? parseFloat(targetPrice) : null,
          stop_loss: stopLoss.trim() ? parseFloat(stopLoss) : null,
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
          target_price: targetPrice.trim() ? parseFloat(targetPrice) : null,
          stop_loss: stopLoss.trim() ? parseFloat(stopLoss) : null,
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
    <div className="modal-backdrop open" onClick={closeNoteModal}>
      <div className="note-modal" onClick={e => e.stopPropagation()}>
        <div className="note-modal-header">
          <div className="note-modal-title-row">
            <span className="note-modal-icon">📝</span>
            <span className="note-modal-title">
              {editingNote ? 'Edit Analysis Note' : 'Log Analysis Note'}
            </span>
          </div>
          <button className="note-modal-close" onClick={closeNoteModal}>
            &times;
          </button>
        </div>

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

        <form onSubmit={handleSave}>
          <div className="note-modal-body">
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
                rows={4}
                placeholder="Volume surge, RSI divergence, support level holding..."
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
      </div>
    </div>
  );
};
