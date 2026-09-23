import React, { useCallback, useState } from 'react';
import { useApp } from './context/AppContext';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';
import { LeftRail } from './components/layout/LeftRail';
import { Sidebar } from './components/sidebar/Sidebar';
import { TopBar } from './components/topbar/TopBar';
import { FocusChart } from './components/chart/FocusChart';
import { MultiChartGrid } from './components/grid/MultiChartGrid';
import { IntelDrawer } from './components/drawer/IntelDrawer';
import { StatusStrip } from './components/layout/StatusStrip';
import { NoteModal } from './components/modals/NoteModal';
import { AuditModal } from './components/modals/AuditModal';
import { ShortcutsModal } from './components/modals/ShortcutsModal';
import { CommandPalette } from './components/modals/CommandPalette';
import { Toast } from './components/common/Toast';

export const App: React.FC = () => {
  useKeyboardShortcuts();
  const { viewMode } = useApp();

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
          {viewMode === 'single' ? (
            <FocusChart onPriceUpdate={handlePriceUpdate} />
          ) : (
            <MultiChartGrid />
          )}

          <IntelDrawer />
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
