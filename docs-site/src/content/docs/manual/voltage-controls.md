---
title: Switched shunts and taps
description: Switched shunts holding a voltage or another device's reactive output (MODSW 1–6) and transformer tap control (COD 1) between Newton solves.
---

RAW files describe switched shunts and transformer tap changers that hold bus
voltages within a band, and switched shunts that keep another device's
reactive output within a band. Stannum applies them in AC solves when **Move switched
shunts** and **Move transformer taps** are on (both are on by default). The
case's starting susceptance and tap ratio are the starting point either way.

## Locking them

Turning either setting off **locks** those devices: no shunt switches, or no
tap moves, during the solve. A locked device stays where it is. When the solve
starts from the last solution (the default), that is where the last solve left
it, so a solve with taps moving followed by one with taps locked keeps the
moved taps. With **Flat start** ticked, or `start` set to `"case"` in a run
file or script, it is the value in the case. **Adjust DC converter taps** locks
the HVDC converter taps the same way.

In run files and Python the settings are `switch_shunts`,
`adjust_transformer_taps` and `adjust_dc_taps`; `false` locks.

| Device | RAW data | Supported |
| --- | --- | --- |
| Switched shunt, discrete | `MODSW` = 1, revision 30 and later | Yes |
| Switched shunt, continuous | `MODSW` = 2 | Yes |
| Switched shunt, plant reactive power | `MODSW` = 3 | Yes |
| Switched shunt, VSC converter reactive power | `MODSW` = 4 | Yes |
| Switched shunt, another shunt's setting | `MODSW` = 5 | Yes |
| Switched shunt, FACTS shunt reactive power | `MODSW` = 6 | Yes (see [FACTS devices](/StannumRevival/manual/facts/)) |
| Transformer winding, voltage control | `COD` = 1 | Yes |
| Transformer winding, other controls | `COD` = 2–5 (reactive, active power, DC, asymmetric) | No |

## When they move

Shunts and taps move only between Newton solves, and only after generator
reactive limits have settled, so that they see the final reactive limits
rather than fight them. A device whose controlled bus is held by a generator
(a PV or reference bus) stays put. Each **voltage control pass** moves every
device that needs it and then solves again; after 30 passes the devices are
held.

If the solve after a pass fails, the pass is undone and the devices it moved
are held, and the solve continues from the saved state.

## Discrete shunts

A discrete shunt (MODSW 1) switches **one block at a time** while its
controlled bus is outside `VSWLO`…`VSWHI` (with a $10^{-5}$ p.u. tolerance):
the next block up when the voltage is low, the next block down when it is
high. The block levels come from the shunt's blocks in RAW order.

A device that reverses direction twice is held, so it cannot hunt between two
blocks.

## Shunts that watch another device

In modes 3 to 6 a switched shunt switches whole blocks, like a discrete shunt,
to keep **another device's reactive output** within a band, so that device
keeps reactive reserve. `VSWLO` and `VSWHI` are then fractions (0 to 1) of
that device's whole reactive range, measured from its bottom:

$$
Q_\text{low} = Q_{\min} + V_\text{SWLO}\,(Q_{\max} - Q_{\min}), \qquad
Q_\text{high} = Q_{\min} + V_\text{SWHI}\,(Q_{\max} - Q_{\min}).
$$

| Mode | Watches (`SWREM`) | Range |
| --- | --- | --- |
| 3 | The plant at that bus: all its online generators together | Sum of their `QB`…`QT` |
| 4 | The converter at that bus of the VSC line named in `RMIDNT` | The converter's `MINQ`…`MAXQ` |
| 5 | The switched shunt at that bus (in revision 35, the one whose ID is `RMIDNT` when given) | That shunt's range, from all its reactor blocks to all its capacitor blocks |
| 6 | The shunt element of the [FACTS device](/StannumRevival/manual/facts/) named in `RMIDNT` (at that bus, when given) | −`SHMX`·`VSET` to +`SHMX`·`VSET` |

When the watched output is **above** the band (it supplies too much), the next
block up switches in; when it is **below**, the next block down. Each pass
moves one block, with a 0.01 Mvar tolerance, and the same hunting rules as
discrete shunts. Unlike voltage control, the shunt still acts when the watched
plant sits at a reactive limit.

After each step the solver checks that it worked: if the watched output fell
by less than 0.05 Mvar per Mvar switched in (the plant is far away, or a nearer
generator holds the voltage), the step is put back and the shunt is held, so it
does not run to the end of its range for nothing.

A shunt is held at its starting value when what it watches cannot respond: a
plant with no online generator, a VSC converter that is off or in power factor
control, a FACTS device whose shunt element is held, or a watched shunt that
is itself held or locked. A RAW record whose watched VSC line, shunt or FACTS
device is not in the case is frozen at `BINIT` with an import warning.

## Continuous shunts

A continuous shunt (MODSW 2) behaves like a static var compensator. It is
solved as a reactive source: a helper generator at its bus that holds the
middle of its band, $V_\text{mid} = (V_\text{SWLO} + V_\text{SWHI})/2$, and
injects the change from the starting susceptance $B_0$, within

$$
Q_{\min} = (B_{\min} - B_0)\,V_\text{mid}^2, \qquad Q_{\max} = (B_{\max} - B_0)\,V_\text{mid}^2,
$$

where $B_{\min}$ and $B_{\max}$ span the range of its blocks. A continuous
shunt at a bus that already has an online generator stays at its starting
value. Its limits are always enforced, even when
generator Q limits are not. Once it reaches an end of its range it becomes a
**fixed shunt** there, so its output then follows the voltage squared as a
real shunt's would.

## Tap changers

A transformer winding in voltage control (COD 1) steps its ratio in whole
positions while the controlled bus (`CONT`) is outside `VMI`…`VMA`.

- **Target.** It aims 20% of the band inside the nearer edge, as tap changers
  do: $V^\text{target} = V_{\min} + 0.2\,(V_{\max} - V_{\min})$ when low,
  $V_{\max} - 0.2\,(V_{\max} - V_{\min})$ when high.
- **Step size.** The ratio step is $(R_{\max} - R_{\min})/(N_{tp} - 1)$ from
  `RMA`, `RMI` and `NTP`. Before a move has been measured, the solver assumes a
  stiff source, where the voltage moves by the same fraction as the ratio,
  shared among the taps holding the same bus:

  $$
  \Delta r = \pm\,\frac{V^\text{target} - V}{V}\cdot\frac{r}{n_\text{taps}} .
  $$

  That undershoots, never overshoots. After a move it measures the response
  $s = \Delta V / \Delta r$ and uses $\Delta r = (V^\text{target} - V)/s$.
- **Limits.** At most four positions per pass, never outside `RMI`…`RMA`.
- **Ineffective taps.** A tap whose measured response is below
  $0.05$ (relative voltage change per relative ratio change) barely moves its
  bus, usually because the controlled bus is on the stiff side; it is held.
- **Hunting.** Like shunts, a tap that reverses direction twice is held.

## Results

The overview counts how many shunts and taps moved from their starting
values, and the log lists each move with the bus voltage that caused it.
[Sensitivities](/StannumRevival/manual/sensitivities/) treat the final shunt
and tap settings as fixed.

On SyntheticUSA (150 continuous shunts, 291 taps) the reactive-limit solve
converges with both on, and the largest bus voltage error against the saved
solution drops from 0.076 to 0.047 p.u. On the 993-bus Memphis case
(61 continuous shunts) the mean error drops from 0.0013 to 0.00006 p.u.
