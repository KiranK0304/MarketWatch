import { useEffect } from 'react';
import { useApp } from '../context/AppContext';

export function useKeyboardShortcuts() {
  const {
    setTimeframe,
    nextStock,
    prevStock,
    triggerForceRefresh,
    openNoteModal,
    closeNoteModal,
    isNoteModalOpen,
    isAuditModalOpen,
    closeAuditModal,
    isShortcutsOpen,
    openShortcuts,
    closeShortcuts,
    isCommandPaletteOpen,
    openCommandPalette,
    closeCommandPalette,
    toggleDrawer,
    viewMode,
    setViewMode,
  } = useApp();

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // 1. Handle Escape first to close any modal
      if (e.key === 'Escape') {
        if (isCommandPaletteOpen) closeCommandPalette();
        else if (isShortcutsOpen) closeShortcuts();
        else if (isAuditModalOpen) closeAuditModal();
        else if (isNoteModalOpen) closeNoteModal();
        return;
      }

      // 2. Command Palette shortcut (Ctrl+K or Cmd+K)
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        if (isCommandPaletteOpen) closeCommandPalette();
        else openCommandPalette();
        return;
      }

      // If user is typing in an input or textarea, don't trigger global shortcuts
      const activeEl = document.activeElement;
      if (
        activeEl &&
        (activeEl.tagName === 'INPUT' || activeEl.tagName === 'TEXTAREA' || activeEl.tagName === 'SELECT')
      ) {
        return;
      }

      // If any modal is open, don't trigger background shortcuts
      if (isNoteModalOpen || isAuditModalOpen || isShortcutsOpen || isCommandPaletteOpen) {
        return;
      }

      switch (e.key.toLowerCase()) {
        case '1':
          setTimeframe('5m');
          break;
        case '2':
          setTimeframe('15m');
          break;
        case '3':
          setTimeframe('30m');
          break;
        case '4':
          setTimeframe('1h');
          break;
        case '5':
          setTimeframe('1d');
          break;
        case '6':
          setTimeframe('1w');
          break;
        case 'arrowdown':
        case 'j':
          e.preventDefault();
          nextStock();
          break;
        case 'arrowup':
        case 'k':
          e.preventDefault();
          prevStock();
          break;
        case ' ':
        case 'f':
          e.preventDefault();
          triggerForceRefresh();
          break;
        case 'n':
          e.preventDefault();
          openNoteModal();
          break;
        case 'i':
          e.preventDefault();
          toggleDrawer();
          break;
        case 'g':
          e.preventDefault();
          setViewMode(viewMode === 'single' ? 'grid' : 'single');
          break;
        case '?':
          e.preventDefault();
          openShortcuts();
          break;
        case '/':
          e.preventDefault();
          const searchEl = document.getElementById('search-box') as HTMLInputElement | null;
          searchEl?.focus();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    setTimeframe,
    nextStock,
    prevStock,
    triggerForceRefresh,
    openNoteModal,
    closeNoteModal,
    isNoteModalOpen,
    isAuditModalOpen,
    closeAuditModal,
    isShortcutsOpen,
    openShortcuts,
    closeShortcuts,
    isCommandPaletteOpen,
    openCommandPalette,
    closeCommandPalette,
    toggleDrawer,
    viewMode,
    setViewMode,
  ]);
}
