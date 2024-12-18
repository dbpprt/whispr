import { useEffect } from 'react';
import './App.css';

function App() {
  return (
    <div className="overlay">
      <div className="status-indicator"></div>
      <span className="status-text">Listening...</span>
    </div>
  );
}

export default App;
