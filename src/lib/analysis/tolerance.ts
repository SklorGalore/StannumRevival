// The mismatch tolerance is set in MVA, the way engineers think of it (PSS/E's
// default is 0.1 MVA), and the solver converts it to per unit on each case's
// own base. These convert for display, rounding to 12 digits so float noise
// never shows.

const clean = (x: number) => Number(x.toPrecision(12));

export const toleranceMva = (pu: number, baseMva: number) => clean(pu * baseMva);

export const tolerancePu = (mva: number, baseMva: number) => mva / (baseMva > 0 ? baseMva : 100);
