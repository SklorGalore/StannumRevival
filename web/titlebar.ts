import {isTauri} from '@tauri-apps/api/core';
import {getCurrentWindow} from '@tauri-apps/api/window';

export function initializeTitlebar(root:HTMLElement):void {
  if(!isTauri()||!navigator.platform.startsWith('Win')) return;

  document.documentElement.classList.add('windows-titlebar-enabled');
  root.insertAdjacentHTML('afterbegin',`<div class="desktop-titlebar" aria-label="Window title bar">
    <div class="desktop-titlebar-drag" data-tauri-drag-region><span data-tauri-drag-region>Nodal</span></div>
    <div class="desktop-titlebar-controls">
      <button class="desktop-titlebar-button" type="button" data-window-action="minimize" aria-label="Minimize window" title="Minimize"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 6h8"/></svg></button>
      <button class="desktop-titlebar-button" type="button" data-window-action="maximize" aria-label="Maximize or restore window" title="Maximize or restore"><svg viewBox="0 0 12 12" aria-hidden="true"><rect x="2.5" y="2.5" width="7" height="7"/></svg></button>
      <button class="desktop-titlebar-button close" type="button" data-window-action="close" aria-label="Close window" title="Close"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="m3 3 6 6M9 3 3 9"/></svg></button>
    </div>
  </div>`);

  const window=getCurrentWindow();
  root.addEventListener('click',event=>{
    const button=(event.target as Element).closest<HTMLButtonElement>('[data-window-action]');
    if(!button) return;
    const action=button.dataset.windowAction;
    if(action==='minimize') void window.minimize();
    else if(action==='maximize') void window.toggleMaximize();
    else if(action==='close') void window.close();
  });
}
