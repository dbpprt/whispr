import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';

function App() {
  const [status, setStatus] = useState('');

  useEffect(() => {
    const unlistenStart = listen('status-change', (event) => {
      setStatus(event.payload as string);
    });

    return () => {
      unlistenStart.then((unlistenFn) => unlistenFn());
    };
  }, []);

  return (
    <div className="App">
      <header className="App-header">
        <span className="status-text">{status}</span>
      </header>
    </div>
  );
}

export default App;
