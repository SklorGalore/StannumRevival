---
title: Two-terminal VSC
description: Voltage source converter DC lines - DC-side power balance, converter losses, AC voltage or power factor control, and MVA and current limits.
---

A voltage source converter (VSC) DC line has one converter that holds the DC
voltage (RAW `TYPE` = 1) and one that holds an active power order at its AC
bus (`TYPE` = 2). Units are kV, kA, ohms, MW and Mvar.

## DC side

With $k$ the power converter and $v$ the voltage converter, current $I$
flowing from $k$ to $v$, and $V_v$ = `DCSET` of the voltage end:

$$
V_k - V_v = R_{DC}\,I .
$$

Each converter loses

$$
L = \max\left(A_\text{LOSS} + B_\text{LOSS}\,I,\ \text{MINLOSS}\right),
$$

with RAW's `ALOSS` in kW and `BLOSS` in kW/A. Power balance at the power
converter, whose AC injection is its order $P_k$ (positive feeding the AC
system):

$$
P_k + L_k = -V_k\,I = -(V_v + R_{DC} I)\,I ,
$$

a quadratic in $I$. The voltage converter then injects
$V_v I - L_v$ into its AC bus, so it delivers the order less both converter
losses and the line's $R_{DC}I^2$.

## AC side

Each converter controls its AC side independently:

| `MODE` | Control |
| --- | --- |
| 1 | Holds its AC bus voltage at `ACSET` within `MINQ`…`MAXQ`, like a generator |
| 2 | Holds a fixed power factor, so its Q follows its P |

## Converter limits

After each converged Newton solve, a converter whose apparent power exceeds
`SMAX`, or whose AC current at the solved voltage exceeds `IMAX`, has its
output cut. The cut is split between P and Q by `PWF`: 0 cuts only active
power, 1 only reactive power.

- Active power is cut through the power order (scaled by the kept fraction).
- Reactive power at a power-factor end is cut directly.
- Reactive power at a voltage-control end is cut by tightening its Q limits,
  enforced when **Enforce generator Q limits** is on.

Cuts only shrink, so the limit passes end. The **DC lines** tab shows the kept
fraction of the order and which ends were limited.

## Results

On `t_psse_case3.raw`, line VDCLINE1 delivers 205.29 MW of its 209 MW order
and VDCLINE2 99.19 MW of 100 MW once converter and line losses are included.

## Not modelled

Remote voltage regulation by a converter (`REMOT`, `RMPCT`); import warns.
