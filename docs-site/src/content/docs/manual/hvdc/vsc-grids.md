---
title: Meshed VSC grids
description: Multi-terminal VSC DC grids in MatACDC's model, with DC slack, droop and power control, attached to a case from a MatACDC DC case file.
---

Meshed VSC DC grids use the converter and DC grid model of
[MatACDC](https://www.esat.kuleuven.be/electa/teaching/matacdc/), the
open-source AC/DC power flow from KU Leuven. Stannum reads MatACDC's DC case
files and solves them with its own implementation of the published equations
(Beerten, Cole and Belmans, IEEE Trans. Power Syst. 27(2), 2012).

![VSC grids tab: converter stations, DC bus voltages and DC branch flows of two grids attached to case14](../../../../assets/screens/power-vsc-grids.png)

## Attaching a DC case

1. Open the AC case. MATPOWER cases in version 2 or in the older version 1
   form (as MatACDC's own AC cases are) both work.
2. In **Power flow**, under **VSC DC grids** in the solve settings, choose
   **Attach MatACDC case…** and pick the DC case (`busdc`, `convdc`,
   `branchdc`).
3. Solve. The **VSC grids** tab lists every converter station, DC bus and DC
   branch.

The DC case's `busac_i` numbers must name buses in the AC case. Remove the
grids with the **×** next to the file name. Opening a DC case on its own says
to open its AC case first.

VSC grids have no simplified form, so they are always solved in detail,
whatever **DC line model** says. DC (linear) power flow ignores them.

## Converter stations

From its AC bus $U_s$, each station is a transformer $Z_{tf}$, a filter bus
$U_f$ with susceptance $B_f$, a phase reactor $Z_c$, and the converter voltage
$U_c$. The transformer, the filter and either resistance can be left out.
Given the bus voltage and the power the station injects into the AC bus,
$S_s = P_s + jQ_s$,

$$
\begin{aligned}
U_f &= U_s + Z_{tf}\,\left(S_s / U_s\right)^{*} \\
S_{cf} &= U_f \left(S_s / U_s\right)^{*} - jB_f\,|U_f|^2 \\
U_c &= U_f + Z_c\,\left(S_{cf} / U_f\right)^{*}, \qquad S_c = U_c \left(S_{cf}/U_f\right)^{*}.
\end{aligned}
$$

The valves lose $P_{loss} = a + b\,I_c + c\,I_c^2$ with $I_c = |S_c| / |U_c|$
(`LossA` in MW, `LossB` in kV, `LossCrec` / `LossCinv` in ohms on the
converter's AC base). As in MatACDC, `LossCrec` applies while the converter
delivers power to the AC side and `LossCinv` while it takes power from it.
The converter takes $P_{dc} = P_c + P_{loss}$ from the DC grid.

## Controls

| `type_dc` | DC side | |
| --- | --- | --- |
| 1 | Power | Holds $P_s$ (`P_g`, MW into the AC grid). |
| 2 | DC slack | Holds its DC bus voltage at `Vdc`. |
| 3 | Droop | $P_{dc} = P_{dc,set} + (U_{dc} - U_{dc,set}) / k$ outside a deadband of ±`dVdcset` around `Vdcset`, and $P_{dc,set}$ inside it; $k$ (`droop`) is in p.u. DC voltage per MW. |

| `type_ac` | AC side |
| --- | --- |
| 1 | Holds $Q_s$ (`Q_g`, Mvar into the AC grid). |
| 2 | Holds its AC bus voltage at `Vtar`, through a generator with unlimited reactive power. On a bus whose voltage a generator already holds, it holds $Q_s = 0$ instead, as in MatACDC. |

Every DC grid needs a slack or droop converter. In a grid with more than one
slack, the one on the lowest-numbered DC bus keeps the voltage and the others
hold their power orders.

## DC grids

Each grid is a resistive network. The power into the grid at DC bus $i$ is

$$
P_i = p\,U_i \sum_j G_{ij}\,U_j,
$$

with $p$ the number of poles (`pol`: 1 for a monopole, 2 for a bipole or a
symmetrically grounded monopole). It is solved by Newton's method on the DC
bus voltages, with the slack's voltage held. Buses without a converter carry
no power.

## The AC/DC solve

As in MatACDC, the AC and DC sides are solved in turn:

1. Starting powers: power orders, droop setpoints, and the slack taking up the
   rest of its grid without losses.
2. Solve the AC network with each converter as a fixed injection.
3. From the AC bus voltages, work out each station, then solve each DC grid.
4. Give the slack and droop converters the AC power that takes the power the
   DC grid needs from them, holding $U_s$ and $Q_s$.
5. Repeat from step 2 until those powers change by less than the mismatch
   tolerance, or $10^{-6}$ p.u. if that is smaller, in at most 30 passes.

Separate AC zones (islands, each with its own reference bus) connected only
through DC grids are solved together.

## Results

The **VSC grids** tab reports, for each converter, its controls, $P_s$, $Q_s$,
converter voltage and angle, valve loss, station loss (everything between
the AC bus and the DC bus), and $P_{dc}$; for each DC bus its voltage and the
converter's $P_{dc}$; and each DC branch's flows and loss. The overview's
total losses include the stations and DC branches. The solved case keeps its
own loads and generators: converter injections appear only in the tab.

## Verification

On MatACDC's own test cases, Stannum matches the results printed in the
MatACDC 1.0 manual: the three-terminal slack case to every printed digit
(DC voltages 1.008, 1.000 and 0.998 p.u., the slack converter's 20.76 MW and
7.14 Mvar, each station's losses and DC branch flows), the point-to-point
case to the four printed decimals (56.4959 MW, 0.0041 Mvar), and both
converter-outage cases. MatACDC may not be redistributed, so these cases are
not in the repository; the tests run when `STANNUM_MATACDC` points at an
unpacked copy of MatACDC 1.0.

## Not modelled

- Converter current and voltage limits (`Imax`, `Vmmax`, `Vmmin`) and DC
  voltage limits.
- DC branch inductance and capacitance, and DC bus capacitance (dynamics
  only).
- MatACDC's infinite AC buses (`inf` bus type).
- Sensitivities treat the converters as fixed injections.
