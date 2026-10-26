---
title: Network model
description: Per-unit system, bus types, the branch π model with taps and phase shifters, shunts, loads and islands.
---

Stannum's power flow works on a bus-branch model with MATPOWER's conventions.
Both MATPOWER and RAW files are converted into it; see
[PSS/E RAW](/StannumRevival/reference/psse-raw/) for the RAW conversion.

## Units

- Powers in the case and results are in MW and Mvar; the solver works in per
  unit on the system base $S_\text{base}$ (`baseMVA`, usually 100 MVA).
- Voltage magnitudes are in per unit of the bus base kV.
- Angles are in degrees in results, radians inside the solver.

## Buses

Each bus has a type:

| Type | Known | Solved for |
| --- | --- | --- |
| Reference (slack) | $\lvert V\rvert$, $\theta$ | $P$, $Q$ |
| PV (voltage controlled) | $P$, $\lvert V\rvert$ | $Q$, $\theta$ |
| PQ | $P$, $Q$ | $\lvert V\rvert$, $\theta$ |
| Isolated (type 4) | excluded with its equipment | — |

A voltage-controlled bus with no online generator is treated as PQ. Each
connected island must have exactly one reference bus with an online
generator; islands are solved together but keep their own angle reference and
slack.

## Branches

Every line and transformer is a π model with a series impedance
$z_s = r + jx$, total line charging $b$, and an ideal transformer with complex
ratio $N = \tau e^{j\theta_\text{shift}}$ at the **from** end (MATPOWER's
convention; a zero $\tau$ means unity). With $y_s = 1/z_s$:

$$
\begin{bmatrix} I_f \\ I_t \end{bmatrix}
=
\begin{bmatrix}
\dfrac{y_s + j\frac{b}{2}}{\tau^2} & -\dfrac{y_s}{\tau e^{-j\theta_\text{shift}}} \\[2ex]
-\dfrac{y_s}{\tau e^{j\theta_\text{shift}}} & y_s + j\dfrac{b}{2}
\end{bmatrix}
\begin{bmatrix} V_f \\ V_t \end{bmatrix}
$$

Three-winding transformers become three two-winding branches to a star bus.
Parallel branches and offline equipment are allowed; offline branches keep
their row with zero flow.

## Shunts and loads

Bus shunts $G_s + jB_s$ are given in MW and Mvar consumed at 1 p.u. and enter
the admittance matrix as $(G_s + jB_s)/S_\text{base}$.

Loads are voltage dependent (ZIP). A bus's load at voltage magnitude $|V|$ is

$$
S_d = (P_d + jQ_d) + (P_I + jQ_I)\,|V| + (P_Z + jQ_Z)\,|V|^2,
$$

with each part given in MW and Mvar consumed at 1 p.u.: constant power
(RAW `PL`, `QL`), constant current (`IP`, `IQ`) and constant impedance
(`YP`, and `YQ` negated, since PSS/E counts capacitive load as positive).
The constant-impedance part enters the admittance matrix like a shunt,
$(P_Z - jQ_Z)/S_\text{base}$. The constant-current part adds
$(P_I + jQ_I)/S_\text{base}$ to the Jacobian's $\partial S/\partial |V|$
diagonal, so Newton keeps converging quadratically. DC power flow takes
every part at 1 p.u. MATPOWER cases have constant power only, and MATPOWER
export writes each bus's load as drawn at its voltage.

## Admittance matrix

The bus admittance matrix $Y$ is assembled sparsely from every online branch
and shunt, so that the bus current injections are $I = YV$ and the complex
power injections are

$$
S = V \circ \overline{(YV)},
$$

where $\circ$ is the element-wise product and the bar is the complex
conjugate.

## Generators

Several generators may share a bus. Their combined reactive output is shared
in proportion to their reactive ranges, as MATPOWER does, including zero-width
and infinite ranges. A generator's voltage setpoint applies at the bus it
regulates: its own bus by default, or a remote bus with
[remote regulation](/StannumRevival/manual/remote-regulation/).
