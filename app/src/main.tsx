/** Mounts the shell. */

import { createRoot } from 'react-dom/client';
import { App } from './shell/App';
import './shell/shell.css';
import './shell/visual-overhaul.css';

const mount = document.getElementById('root');
if (!mount) {
  throw new Error('The page carries no mount element');
}
createRoot(mount).render(<App />);
