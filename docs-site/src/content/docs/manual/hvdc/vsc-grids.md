---
title: Meshed VSC grids
description: Meshed and multi-terminal VSC DC grids with slack, power and droop control, solved in sequence with the AC network.
---

A VSC DC grid has DC buses, resistive DC lines between them in any
topology, and voltage source converters that connect DC buses to AC buses.
Several DC grids can be solved at once, and they can link AC grids that are
not synchronised with each other. The grids come from a [Stannum DC grid
file](/StannumRevival/reference/dc-grid-file/) attached to the open case.

The model and the solution method follow J. Beerten, S. Cole and
R. Belmans, "Generalized steady-state VSC MTDC model for sequential AC/DC
power flow algorithms", *IEEE Transactions on Power Systems* 27(2),
pp. 821–829, 2012. Equation numbers on this page refer to that paper. Where
the paper leaves a choice open, this page says what Stannum does.

![VSC grids tab: converters, DC buses and DC lines of a three-terminal grid](../../../../assets/screens/vsc-grids.png)

## Converter station

Each converter is a controllable voltage $U_c$ behind a phase reactor
$Z_c = R_c + jX_c$. The reactor meets a filter susceptance $B_f$ at the
filter bus $U_f$, and a transformer $Z_{tf}$ connects the filter bus to the
AC system bus $U_s$. Any of the three can be left out. Set-points are
defined at the system bus: $S_s = P_s + jQ_s$ is the power the station
injects into the AC grid.

Given $U_s$ and $S_s$, the station's state follows directly:

$$
\begin{aligned}
I_s &= \left(S_s / U_s\right)^* &
U_f &= U_s + Z_{tf} I_s && (36)\\
I_c &= I_s + jB_f U_f &
U_c &= U_f + Z_c I_c && (37, 38)\\
S_c &= U_c I_c^* & & && (3, 4)
\end{aligned}
$$

$S_c = P_c + jQ_c$ is the power leaving the converter towards the AC side.
The converter loss is

$$
P_\text{loss} = a + b\,I_c + c\,I_c^2 \qquad (10)
$$

with $I_c$ the converter current in kA, $a$ in MW, $b$ in kV and $c$ in
ohms. $c$ takes the rectifier value while the converter takes power from the
AC grid ($P_c < 0$) and the inverter value while it delivers power to it.
The DC power injected into the DC grid is

$$
P_{dc} = -P_c - P_\text{loss}. \qquad (39)
$$

## DC grid

DC lines are resistances. With $p$ poles (1 for an asymmetric monopole, 2
for a symmetric monopole or bipole), the power DC bus $i$ sends into the
grid is

$$
P_i = p\,U_i \sum_j \frac{U_i - U_j}{R_{ij}}, \qquad (30)
$$

with pole-to-ground voltages and resistances per pole conductor.

## Controls

On its DC side each converter is one of:

| Control | Holds | Notes |
| --- | --- | --- |
| Power | $P_s$ at its AC system bus | The paper's constant-$P$ converter. |
| Slack | Its DC bus voltage | The paper's DC slack: its power balances the grid and its losses. |
| Droop | $-P_{dc} = P_\text{set} + k\,(U - U_\text{set})$ | $k$ in MW per kV of its DC bus voltage. |

The paper cites droop control without giving its equations, so Stannum
defines it on the converter's DC side: a droop converter sends more power out
of the DC grid as its DC voltage rises. Several droop converters share a
grid's balance in proportion to their gains, and a grid can run on droop
converters alone. Stannum's choice to put the droop on the DC side means
$P_\text{set}$ is DC power; the AC injection differs from it by the converter
loss.

On its AC side a converter holds either a reactive power $Q_s$ or the AC
voltage magnitude at its system bus. A voltage-controlling converter makes
its bus a PV bus. It needs a bus without generators that is not the
reference bus, so the reactive power at that bus is the converter's alone.

## Sequential solve

The AC and DC networks are solved in turn (Fig. 5):

1. **AC network.** Each converter is a fixed $P_s + jQ_s$ injection, or a
   PV bus for AC voltage control. Every AC island needs its own reference
   bus. In the first iteration, slack converters share their grid's
   lossless balance (35) and droop converters start at $P_\text{set}$.
2. **Converter limits** (below).
3. **Power-controlled converters.** $P_{dc}$ from the station equations at
   the AC solution.
4. **DC networks.** Each grid's bus voltages by Newton's method (45)–(48),
   with slack voltages fixed and droop converters' power following their
   bus voltages. The DC Newton runs to $10^{-12}$ p.u. with at most 50
   iterations.
5. **Slack and droop converters.** The $P_s$ that delivers their DC power,
   holding $U_s$ and $Q_s$ at the AC solution (Fig. 6). The paper does this
   with a four-variable Newton iteration on the converter and filter
   voltages. Because the station equations above give $P_{dc}$ explicitly
   for any $P_s$, Stannum solves the same equations with a one-variable
   Newton iteration on $P_s$.
6. **Repeat** until no converter's $P_s$ or $Q_s$ moves by more than the
   solve's mismatch tolerance between iterations, at most 30 times.

Each AC solve starts from the previous one's voltages and uses the solve
settings as usual, including generator reactive limits, switched shunts,
taps and any DC lines in the case. The case's own two-terminal and
multi-terminal DC lines keep their [detailed models](/StannumRevival/manual/hvdc/overview/).
As in the paper, the sequential method needs few iterations: the demo case
below settles in two.

## Converter limits

Three limits can be given for each converter: a maximum converter current
$|I_c|$ and maximum and minimum converter voltages $|U_c|$. At a known
system bus voltage, each is a circle in the $P_s Q_s$ plane (15), (23):

$$
\begin{aligned}
\text{current:}\quad S_0 &= -U_s^2\,\frac{Y_f^*}{1 + Z_{tf}^* Y_f^*}, &
r &= \frac{U_s I_{c,\max}}{|1 + Z_{tf} Y_f|} && (41, 42)\\
\text{voltage:}\quad S_0 &= -U_s^2\,(Y_1 + Y_2)^*, &
r &= U_s U_{c,m}\,|Y_2| && (43, 44)
\end{aligned}
$$

where $Y_f = jB_f$ and $Y_1$, $Y_2$ are the shunt and series admittances of
the station's π-equivalent (18)–(20), written so that they stay finite
without a filter or transformer:

$$
Y_2 = \frac{1}{Z_{tf} Z_c Y_f + Z_c + Z_{tf}}, \qquad Y_1 = Z_c Y_f\,Y_2.
$$

With $P_s$ known, the circles bound $Q_s$ (40): the current circle from
above and below, the upper voltage circle from above, and the lower voltage
circle from below.

How Stannum applies them, with active power first as in the paper:

- A **power order** outside the current or upper voltage circle runs at the
  circle's edge instead, recalculated at every iteration, so it returns to
  the full order when the AC voltage allows.
- A **reactive set-point** outside its range is held at the nearest limit,
  also recalculated each iteration.
- A **voltage-controlling converter** that needs more reactive power than
  its range allows holds that limit for the rest of the solve, like a
  generator at a reactive limit. Its bus becomes a PQ bus.
- **Slack and droop converters** can't be moved without upsetting their
  grid's balance, so a violation there is reported (**Outside limits**), not
  corrected.

The paper found that applying limits at every iteration could keep the
solution from settling, and only updated set-points after large enough
changes. Stannum doesn't damp: a limited converter keeps its limited state
and only the limit value moves with the AC voltage. In the tests this
settles in a few iterations.

## Verification

- **The paper's results.** From the system-bus $P_s$, $Q_s$, $U_s$ and
  $\delta_s$ in Tables II and III, the station equations reproduce all seven
  converters' $P_c$, $Q_c$, $U_c$, $\delta_c$, $P_\text{loss}$ and
  $P_{dc}$ to the tables' rounding. Two readings of Table I are needed for
  the losses: the coefficient printed as $c_\text{rec}$ matches the
  converters that deliver power to the AC grid and $c_\text{inv}$ the one
  that takes power from it, and the constant $a$ of the 200 MW converters 4
  and 6 is 2.206 MW (the 100 MW converters 5 and 7 use 1.103 MW).
- **Slack iteration.** $P_s \to P_{dc} \to P_s$ round trips to $10^{-9}$,
  and the paper's DC slack (converter 1) gives its $P_s$ of 66.84 MW from
  its $P_{dc}$ of −68.47 MW.
- **Limits.** On each circle's edge the station equations give exactly
  $I_{c,\max}$, $U_{c,\max}$ or $U_{c,\min}$, with and without a filter.
- **DC network.** Hand solutions of a radial grid with one and two poles,
  and the droop law at a solution with only droop converters.
- **Whole solve.** A case with three asynchronous AC grids and two meshed
  DC grids (below): each grid's converters put in exactly its line losses,
  line currents and losses match the bus voltages, every converter holds its
  set-points, and limits behave as described above.
- **Benchmark against MatACDC.** [MatACDC](https://www.esat.kuleuven.be/electa/teaching/matacdc/)
  (J. Beerten, KU Leuven) is the open-source AC/DC power flow that
  accompanies the paper. MatACDC 1.0 solved the demo case and two
  variations of it, converted to its input format by
  `scripts/matacdc_benchmark.m`, and its results are stored with the tests.
  MatACDC is used only to check results: Stannum's model is written from
  the paper, and neither MatACDC's code nor its cases are part of Stannum.

  | Case | Largest difference |
  | --- | --- |
  | Demo case: slack, droop, power and AC voltage control | $P_s$ 6·10⁻⁸ MW, $Q_s$ 5·10⁻⁹ Mvar, DC voltage 2·10⁻¹⁵ p.u., AC voltage 9·10⁻¹² p.u. |
  | Converter A2 out, the two droop converters take up its 60 MW | $P_s$ 9·10⁻⁸ MW, DC voltage 1·10⁻¹⁵ p.u. |
  | South ordered to 120 MW against a 1.0 p.u. current limit, Offshore's converter voltage capped at 1.08 p.u. | South 101.85 MW against 100.00 MW, North 1.87 MW lower to balance, Offshore $Q_s$ 0.60 Mvar |

  With limits the two differ by design. Stannum puts a limited converter
  exactly on its limit circle (section III-C of the paper), so South runs
  at exactly 1.0 p.u. current and Offshore at exactly 1.08 p.u. converter
  voltage. MatACDC 1.0 stops inside the limits, at 0.98 p.u. and 1.078 p.u.
  The four-terminal grid, which reaches no limit, agrees as in the other
  cases.

## Demo case

`crates/stannum-core/tests/fixtures/vsc_grids/` has a demo: `two_area.m`
is two asynchronous copies of MATPOWER's 9-bus case and an offshore wind
island, and `two_area.dcgrid.json` links them with a meshed three-terminal
±150 kV grid (a DC slack, an AC-voltage-controlling converter and the
offshore converter) and a meshed four-terminal ±300 kV grid with two droop
converters. The converter data follows the paper's Table I.

## Not modelled

- A converter that sets the frequency and angle of an AC island with no
  generator (a grid-forming offshore converter). Give such an island a
  reference bus.
- Converter outages beyond `in_service`, and DC breakers.
- Limits on slack and droop converters are reported, not enforced.
- Sensitivities with VSC grids, and a unified AC/DC Newton.
