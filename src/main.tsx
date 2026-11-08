import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import './styles/global.css';

const root = document.getElementById('root');
if (!root) throw new Error('Không tìm thấy vùng hiển thị chính của Zone.');

function suppressBrowserMenu(event: MouseEvent) {
  // Keep the native editing menu for writable text fields.
  if (!(event.target instanceof Element && event.target.matches(':read-write'))) {
    event.preventDefault();
  }
}

document.addEventListener('contextmenu', suppressBrowserMenu, true);
import.meta.hot?.dispose(() => {
  document.removeEventListener('contextmenu', suppressBrowserMenu, true);
});

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
