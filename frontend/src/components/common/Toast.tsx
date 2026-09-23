import React from 'react';
import { useApp } from '../../context/AppContext';

export const Toast: React.FC = () => {
  const { toast } = useApp();

  return (
    <div className={`toast ${toast ? 'show' : ''}`} id="toast">
      {toast || ''}
    </div>
  );
};
