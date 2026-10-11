export const iconNames = ['overview', 'islands', 'bus', 'critical', 'equipment', 'diagnostics', 'open', 'search', 'trace', 'sun', 'moon', 'monitor'] as const;
export type IconName = typeof iconNames[number];

const paths: Record<IconName, string> = {
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>',
  moon: '<path d="M20.5 14.5A9 9 0 0 1 9.5 3.5a9 9 0 1 0 11 11z"/>',
  monitor: '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M12 17v4m-4 0h8"/>',
  overview: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
  islands: '<circle cx="7" cy="7" r="3"/><circle cx="17" cy="7" r="3"/><circle cx="12" cy="17" r="3"/><path d="M9.5 8.5l1.5 6m3.5-6L13 14"/>',
  bus: '<circle cx="12" cy="12" r="7"/><path d="M12 1v4m0 14v4M1 12h4m14 0h4"/>',
  critical: '<path d="M12 3l9 17H3L12 3z"/><path d="M12 9v4m0 3v.1"/>',
  equipment: '<path d="M4 7h6m4 0h6M4 17h6m4 0h6"/><circle cx="12" cy="7" r="2"/><circle cx="12" cy="17" r="2"/>',
  diagnostics: '<path d="M5 4h14M5 10h14M5 16h9"/><circle cx="18" cy="17" r="3"/>',
  open: '<path d="M3 7V4h7l2 2h9v14H3z"/><path d="M12 10v7m-3-3 3 3 3-3"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
  trace: '<circle cx="5" cy="18" r="2"/><circle cx="19" cy="6" r="2"/><path d="M7 18h4l6-12h2"/>',
};

export function icon(name: IconName): string {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${paths[name]}</svg>`;
}
