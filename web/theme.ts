export type ThemePreference='system'|'light'|'dark';

// Run before the app renders so the first frame uses the selected palette.
const systemTheme=window.matchMedia('(prefers-color-scheme: dark)');
let preference:ThemePreference='system';
try {
  const saved=localStorage.getItem('nodal-theme');
  if(saved==='light'||saved==='dark') preference=saved;
} catch { /* The system default works even when storage is blocked. */ }

function applyTheme():void {
  document.documentElement.dataset.theme=preference==='system'?(systemTheme.matches?'dark':'light'):preference;
}
applyTheme();
systemTheme.addEventListener('change',()=>{
  if(preference==='system') {
    applyTheme();
    document.dispatchEvent(new Event('themechange'));
  }
});

export function initializeTheme(control:HTMLElement):void {
  const buttons=control.querySelectorAll<HTMLButtonElement>('[data-theme-preference]');
  const updateButtons=()=>{
    for(const button of buttons) button.setAttribute('aria-pressed',String(button.dataset.themePreference===preference));
  };
  updateButtons();
  control.addEventListener('click',event=>{
    const button=(event.target as Element).closest<HTMLButtonElement>('[data-theme-preference]');
    if(!button||!control.contains(button)) return;
    const value=button.dataset.themePreference;
    if(value!=='light'&&value!=='dark'&&value!=='system') return;
    preference=value;
    try {localStorage.setItem('nodal-theme',preference);} catch { /* Keep the current session usable. */ }
    applyTheme();
    updateButtons();
    document.dispatchEvent(new Event('themechange'));
  });
}
