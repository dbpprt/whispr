import { useState, useEffect, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';

function App() {
  const [isActive, setIsActive] = useState(false);
  const [heights, setHeights] = useState(() => 
    Array.from({ length: 35 }, () => 0.2)
  );

  useEffect(() => {
    let animationFrame: number;

    let time = 0;
    const animate = () => { 
      time += 0.02;
      setHeights(prevHeights => 
        prevHeights.map((_, i) => {
          // Base wave pattern
          const baseWave = Math.sin(i * 0.15 + time) * 0.3;
          // Secondary wave for complexity
          const secondWave = Math.sin(i * 0.1 - time * 0.7) * 0.15;
          // Random variation
          const noise = Math.sin(time * 0.3 + i * 2) * 0.1;

          if (isActive) {
            return 0.4 + baseWave + secondWave + noise;
          }
          return 0.2 + (baseWave + secondWave + noise) * 0.3;
        })
      );
      animationFrame = setTimeout(() => {
        animationFrame = requestAnimationFrame(animate);
      }, 15); // Faster animation
    };

    animate();
    return () => {
      if (animationFrame) {
        clearTimeout(animationFrame);
        cancelAnimationFrame(animationFrame);
      }
    };
  }, [isActive]);

  useEffect(() => {
    const unlistenStart = listen('status-change', (event) => {
      const newStatus = event.payload as string;
      setIsActive(newStatus !== '');
    });

    return () => {
      unlistenStart.then((unlistenFn) => unlistenFn());
    };
  }, []);

  return (
    <div className={`App ${isActive ? 'active' : ''}`}>
      <img src="/src/icon.png" className="tauri-icon" alt="Tauri logo" />
      <div className="waveform-container">
        <div className="waveform">
          {heights.map((height, i) => (
            <div
              key={i}
              className={`bar ${isActive ? 'active' : ''}`}
              style={{
                height: `${height * 100}%`,
                transform: `scaleY(${height})`
              }}
            />
          ))}
        </div>
      </div>
      <div className="microphone">
        <svg viewBox="0 0 24 24" width="24" height="24">
          <path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5.91-3c-.49 0-.9.36-.98.85C16.52 14.2 14.47 16 12 16s-4.52-1.8-4.93-4.15c-.08-.49-.49-.85-.98-.85-.61 0-1.09.54-1 1.14.49 3 2.89 5.35 5.91 5.78V20c0 .55.45 1 1 1s1-.45 1-1v-2.08c3.02-.43 5.42-2.78 5.91-5.78.1-.6-.39-1.14-1-1.14z" />
        </svg>
      </div>
    </div>
  );
}

export default App;
