import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { loadWindowRadius } from './lib/window-radius'

// Once per window, before the first paint if the backend answers in time.
void loadWindowRadius()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
