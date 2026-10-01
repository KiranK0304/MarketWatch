import React, { useCallback, useState } from 'react';
import { useApp } from './context/AppContext';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';
import { LeftRail } from './components/layout/LeftRail';
import { Sidebar } from './components/sidebar/Sidebar';
import { TopBar } from './components/topbar/TopBar';
import { FocusChart } from './components/chart/FocusChart';
import { MultiChartGrid } from './components/grid/MultiChartGrid';
import { JournalStudio } from './components/journal/JournalStudio';
import { IntelDrawer } from './components/drawer/IntelDrawer';
import { StatusStrip } from './components/layout/StatusStrip';
import { NoteModal } from './components/modals/NoteModal';
import { AuditModal } from './components/modals/AuditModal';
import { ShortcutsModal } from './components/modals/ShortcutsModal';
import { CommandPalette } from './components/modals/CommandPalette';
import { Toast } from './components/common/Toast';

export const App: React.FC = () => {
  useKeyboardShortcuts();
  const { viewMode, isSidebarOpen, toggleSidebar, isDrawerOpen, toggleDrawer } = useApp();

  const [heroPrice, setHeroPrice] = useState('₹--');
  const [heroChange, setHeroChange] = useState({ text: '--', isPositive: true });

  const handlePriceUpdate = useCallback((price: string, change: { text: string; isPositive: boolean }) => {
    setHeroPrice(price);
    setHeroChange(change);
  }, []);

  return (
    <>
      <LeftRail />
      <Sidebar />

      <main className="main-stage">
        <TopBar heroPrice={heroPrice} heroChange={heroChange} />

        <div className="viewport-area">
          {!isSidebarOpen && (
            <button
              type="button"
              className="panel-edge-tab left"
              id="btn-open-sidebar-floating"
              onClick={toggleSidebar}
              title="Open Left Sidebar (B)"
            >
              ▶
            </button>
          )}

          {viewMode === 'single' ? (
            <FocusChart onPriceUpdate={handlePriceUpdate} />
          ) : viewMode === 'grid' ? (
            <MultiChartGrid />
          ) : (
            <JournalStudio onPriceUpdate={handlePriceUpdate} />
          )}

          {viewMode !== 'journal' && <IntelDrawer />}

          {viewMode !== 'journal' && !isDrawerOpen && (
            <button
              type="button"
              className="panel-edge-tab right"
              id="btn-open-drawer-floating"
              onClick={toggleDrawer}
              title="Open Intel & AI Drawer (I)"
            >
              ◀
            </button>
          )}
        </div>

        <StatusStrip />
      </main>

      {/* Modals and Overlays */}
      <NoteModal />
      <AuditModal />
      <ShortcutsModal />
      <CommandPalette />
      <Toast />
    </>
  );
};
