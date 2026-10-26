---
title: Two-terminal LCC
description: Six-pulse bridge equations, rectifier and inverter control modes, mode switching, tap adjustment and reactive power of line-commutated DC lines.
---

A two-terminal line-commutated converter (LCC) DC line has a rectifier, an
inverter and a DC line between them. Stannum solves it with the PSS/E data
model in PSS/E units: kV, kA, ohms, MW and Mvar.

## Converter equations

For a converter with $N_B$ six-pulse bridges in series, AC bus voltage $E$ in
kV (per-unit voltage times the base `EBAS`), transformer ratio $T_R$ and tap
$T$, the no-load DC voltage is

$$
V_{do} = \frac{3\sqrt{2}}{\pi}\,N_B\,\frac{E\,T_R}{T}.
$$

With commutating reactance $X_c$ and resistance $R_c$ per bridge and DC
current $I_d$:

$$
\begin{aligned}
V_{dr} &= V_{do,r}\cos\alpha - N_{B,r}\left(\tfrac{3}{\pi}X_{c,r} + 2R_{c,r}\right) I_d && \text{rectifier} \\
V_{di} &= V_{do,i}\cos\gamma - N_{B,i}\left(\tfrac{3}{\pi}X_{c,i} - 2R_{c,i}\right) I_d && \text{inverter} \\
V_{dr} - V_{di} &= R_{DC}\, I_d && \text{DC line}
\end{aligned}
$$

where $\alpha$ is the rectifier firing angle and $\gamma$ the inverter
extinction angle. Two phases conduct at a time, so each bridge loses
$2R_c I_d^2$.

### Power and reactive power

Each converter's AC-side active power equals its DC power plus its own
losses. Reactive power uses the usual power-factor approximation
$\cos\varphi = V_d'/V_{do}$, where $V_d'$ is the DC voltage before the
resistive drop:

$$
Q = I_d\sqrt{V_{do}^2 - V_d'^2}.
$$

Both converters **absorb** this reactive power from the AC system.

## Controls

The control mode comes from `MDC`: blocked (0), power order (1, `SETVL` in MW)
or current order (2, `SETVL` in amps). A positive power order is measured at
the rectifier, a negative one at the inverter.

| Situation | Rectifier | Inverter |
| --- | --- | --- |
| Normal | Holds the order with $\alpha$ | Runs at minimum $\gamma$ (`ANMNI`) |
| Rectifier at minimum $\alpha$ (`ANMNR`) | Fixed at $\alpha_{\min}$ | Takes current control at the order less the margin `DELTI` |
| Compounded DC voltage below `VCMOD` (power order) | The order becomes the current `SETVL / VSCHD` | |

The DC voltage is held at the **compounding point**, `RCOMP` ohms from the
inverter towards the rectifier:

$$
V_\text{comp} = V_{di} + R_\text{COMP}\, I_d .
$$

## Inside the AC solve

The operating point is recomputed from the two terminal voltage magnitudes
at every Newton step. The converter P and Q enter the mismatch as loads, and
their analytic derivatives with respect to the terminal voltage magnitudes
enter the Jacobian.

## Tap adjustment

With **Adjust DC converter taps**, converter taps step between Newton solves,
like reactive-limit passes:

- the **inverter** tap brings the compounded DC voltage to `VSCHD`;
- the **rectifier** tap keeps $\alpha$ within its nominal range `ANMNR`…`ANMXR`.

Taps move in steps of `STPR`/`STPI` (continuously when the step is zero)
within `TMN`…`TMX`, and never step past the hard minimum angle. After 20 tap
passes they are held.

The maximum angles `ANMX` are treated as the nominal top of the tap band, the
minimum angles as hard limits.

## Results

The **DC lines** tab reports current, DC voltages, $\alpha$, $\gamma$, taps,
control mode, losses, and both ends' P and Q. The case's DC line row receives
the AC-side P and Q (absorbed Q negative) and the terminal voltages.

On the PSS/E test case `t_psse_case3.raw`, the two LCC lines lose 60.5 and
63.8 MW, which the lossless simplified model ignores.

## Not modelled

- Capacitor-commutated converters (`XCAP`).
- The AC transformer named by `IF`/`IT`/`ID`; the tap is applied inside the
  converter instead.
- A separate commutating bus (`IC`).
- `DCVMIN`.

Import warns when a line uses any of them.
