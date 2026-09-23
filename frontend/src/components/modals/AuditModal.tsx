import React, { useState, useEffect } from 'react';
import { useApp } from '../../context/AppContext';
import { api } from '../../api/client';
import type { NoteStatus } from '../../types';

export const AuditModal: React.FC = () => {
  const { isAuditModalOpen, auditNote, closeAuditModal, showToast, triggerForceRefresh } = useApp();

  const [status, setStatus] = useState<NoteStatus>('validated');
  const [outcomeNote, setOutcomeNote] = useState('');
  const [isUpdating, setIsUpdating] = useState(false);

  useEffect(() => {
    if (!isAuditModalOpen || !auditNote) return;
    setStatus(auditNote.status === 'open' ? 'validated' : auditNote.status);
    setOutcomeNote(auditNote.outcome_note || '');
  }, [isAuditModalOpen, auditNote]);

  if (!isAuditModalOpen || !auditNote) return null;

  const handleUpdate = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsUpdating(true);
    try {
      await api.updateNote(auditNote.id, {
        status: status,
        outcome_note: outcomeNote.trim() || null,
      });
      showToast(`Audit saved for Note #${auditNote.id}`);
      triggerForceRefresh();
      closeAuditModal();
    } catch (err: any) {
      showToast(`Failed to update outcome: ${err.message}`);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <div className="modal-backdrop open" onClick={closeAuditModal}>
      <div className="note-modal note-modal-sm" onClick={e => e.stopPropagation()}>
        <div className="note-modal-header">
          <div className="note-modal-title-row">
            <span className="note-modal-icon">⚖️</span>
            <span className="note-modal-title">Verify Outcome</span>
          </div>
          <button className="note-modal-close" onClick={closeAuditModal}>
            &times;
          </button>
        </div>

        <div className="audit-note-summary">
          <div className="audit-note-name">{auditNote.title}</div>
          <div className="audit-note-meta">
            #{auditNote.id} • {auditNote.symbol} ({auditNote.timeframe}) • ₹{auditNote.price_at_note.toFixed(2)}
          </div>
        </div>

        <form onSubmit={handleUpdate}>
          <div className="note-modal-body">
            <div className="note-field">
              <label className="note-field-label">Outcome Status</label>
              <select
                className="note-field-input"
                value={status}
                onChange={e => setStatus(e.target.value as NoteStatus)}
              >
                <option value="validated">✅ Validated (Hit Target / Worked)</option>
                <option value="invalidated">❌ Invalidated (Hit Stop Loss / Failed)</option>
                <option value="open">⏳ Pending / Keep Open</option>
              </select>
            </div>

            <div className="note-field">
              <label className="note-field-label">Retrospective Note</label>
              <textarea
                className="note-field-input note-field-textarea"
                rows={3}
                placeholder="What happened? Did momentum hold? What did we learn?"
                value={outcomeNote}
                onChange={e => setOutcomeNote(e.target.value)}
              />
            </div>
          </div>

          <div className="note-modal-footer">
            <button type="button" className="note-btn note-btn-cancel" onClick={closeAuditModal}>
              Cancel
            </button>
            <button type="submit" className="note-btn note-btn-save" disabled={isUpdating}>
              {isUpdating ? 'Updating...' : 'Update Outcome'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
