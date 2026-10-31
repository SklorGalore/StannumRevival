---
title: Phase shifters
description: Transformer windings that hold their active power flow within a MW band (PSS/E COD 3, symmetric, and COD 5, asymmetric), moved between Newton solves.
---

A phase-shifting transformer changes the angle across a branch to steer
active power through it. RAW files describe one as a transformer winding with
`COD` = 3 (symmetric) or `COD` = 5 (asymmetric): its phase shift angle `ANG`
moves within `RMI`…`RMA` (degrees) to keep the MW flowing into the winding at
its own bus within `VMI`…`VMA`. Stannum applies them in AC solves when **Move
phase shifters** is on (the default). A negative `COD` keeps the winding at
its angle, as in PSS/E. Taps holding a reactive flow (`COD` = 2) move in the
same passes; see [Reactive flow taps](/StannumRevival/manual/voltage-controls/#reactive-flow-taps).

In run files and Python the setting is `adjust_phase_shifters`; `false`
**locks** the shifters, like taps: each keeps its angle where the last solve
left it, or its case angle with **Flat start** ticked. See
[Switched shunts and taps](/StannumRevival/manual/voltage-controls/#locking-them).

## When they move

Phase shifters move between Newton solves, after the generator reactive
limits and the switched shunts and taps have settled. Each **flow control
pass** moves every shifter whose flow is more than 0.1 MW outside its band,
then solves again. After 30 passes they are held; if the solve after a pass
fails, the pass is undone and the shifters it moved are held.

- **Target.** A shifter aims 20% of its band inside the nearer edge, like a
  tap changer: $P^\text{target} = P_{\min} + 0.2\,(P_{\max} - P_{\min})$ when
  the flow is low. A band given high-to-low (`VMA` < `VMI`, as in some
  exported cases) is read as the same band.
- **Moves solved together.** The flows' sensitivities to every moving angle
  come from the power-flow Jacobian at the solved state, so the rest of the
  network's response is included. With $S_{ij} = \partial P_i / \partial
  \varphi_j$ for the shifters outside their bands, the pass solves

  $$
  S\,\Delta\varphi = P^\text{target} - P .
  $$

  Shifters in parallel or in series share the correction instead of
  fighting over it, and most cases settle in one pass.
- **Limits.** Angles stay within `RMI`…`RMA`. When one angle would change by
  more than 10° in a pass, every move in that pass is scaled down together.
  The angle moves continuously; `NTP` is not used.
- **Ineffective or hunting.** A shifter that changes its own flow by less
  than 0.01 MW per degree is held, as is one that reverses direction twice.

The angle follows MATPOWER's sign: the winding's own side leads by the shift,
so a negative angle pushes more power from the winding's bus into the
transformer.

## Asymmetric phase shifters

An asymmetric shifter (`COD` = 5) adds its regulating voltage at the winding
connection angle $\alpha$ (`CNXA`, normalized into −90…90°): the complex ratio
is $1 + k\,e^{j\alpha}$. For a phase shift $\varphi$ its magnitude is

$$
m(\varphi) = \frac{\sin\alpha}{\sin(\alpha - \varphi)},
$$

$1/\cos\varphi$ for a quadrature booster ($\alpha = 90°$). The branch tap
moves with the angle, scaled from the case's own tap at the case's angle:
$t(\varphi) = t_0\,m(\varphi)/m(\varphi_0)$. A connection angle of zero has
no quadrature part, so such a winding stays at its angle with a warning.

## Results

The overview shows **Phase shifters**, how many moved from their case angles,
when the case has any. The Branches table lists every transformer's tap and
phase shift as solved, and the log lists each move with the flow that caused
it.

A winding with an impedance correction table (`TAB`) has its impedance
scaled by the table's factor at its angle, worked out again each time the
shifter moves; the Branches table shows the corrected resistance and
reactance. See [PSS/E RAW](/StannumRevival/reference/psse-raw/).

On SyntheticUSA the two phase shifters in control already sit inside their
bands at the saved solution, and stay put. With their bands moved 400–500 MW
away (to 2,000–2,100 MW and 750–800 MW), both reach them in one flow control
pass.
