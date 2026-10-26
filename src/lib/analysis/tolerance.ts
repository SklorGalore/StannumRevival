// The solver takes its mismatch tolerance in per unit, but engineers think of
// it in MW and Mvar (PSS/E's default is 0.1 MVA). These convert on the case's
// MVA base. The MVA value is rounded to 12 digits so float noise never shows.

const clean = (x: number) => Number(x.toPrecision(12));

export const toleranceMva = (pu: number, baseMva: number) => clean(pu * baseMva);

export const tolerancePu = (mva: number, baseMva: number) => mva / (baseMva > 0 ? baseMva : 100);
