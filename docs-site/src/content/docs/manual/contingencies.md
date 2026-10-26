---
title: Contingency analysis
description: Linear outage screening from one factorization, the AC check of the worst outages, islanding and ranking.
---

Contingency analysis takes each chosen branch or generator out on its own
and checks the network that remains. It starts from the converged AC
solution of the case with the study's edits (the **base**).

## Linear screen

The screen uses the DC network of the base case: susceptances
$b_k = 1 / (x_k \tau_k)$ (taps $\tau_k$ included, phase shifts and
resistance left out, zero reactance taken as $10^{-4}$ p.u.), and the reduced
susceptance matrix $B$ with each island's reference bus removed. $B$ is
factorized once (sparse LU). Every outage then needs one sparse solve per
tripped branch, so the cost grows with the number of outages rather than with
the square of the case, as a full LODF matrix would.

**Branch outages.** For a tripped branch $k$ from bus $f$ to bus $t$, the
solve $B\,\theta = e_f - e_t$ gives the flow $T_{mk}$ on every branch $m$
from a unit transfer across $k$. For a set $K$ of branches tripped together
(the legs of a three-winding transformer), the flows after the outage are

$$
F'_m = F_m + \sum_{j \in K} T_{mj}\, x_j , \qquad (I - T_{KK})\, x = F_K ,
$$

which for one branch is the familiar
$F'_m = F_m + \text{LODF}_{mk} F_k$ with
$\text{LODF}_{mk} = T_{mk} / (1 - T_{kk})$. $F$ are the base case's AC
from-end active flows, so the screen adds linear changes to the AC solution.
Loading after the outage keeps each end's reactive flow and moves both ends'
active flow by the same amount.

**Splitting the network.** When $I - T_{KK}$ is singular (a pivot below
$10^{-6}$), the outage splits an island. The largest part that remains keeps
running, even if it lost its reference bus; every bus outside it is **split
off**. What the tripped boundary branches carried into the remaining part is
lost there and picked up by its slack bus (the reference bus, or else the
bus of the largest remaining generator), and branches inside the split-off
part carry nothing.

**Generator outages.** The generator's output moves to the reference bus of
its island: $\Delta F = -\text{PTDF}_{\cdot,i}\, P_g$, from one solve
$B\,\theta = e_i$.

## AC check

The worst screened outages (or all of them) are solved with the full AC
Newton power flow and the base case's solve settings. Each starts from the
base solution, as in [Starting
point](/StannumRevival/manual/ac-power-flow/#starting-point), with the outage applied:

- tripped branches and generators out of service;
- split-off buses made isolated, with their generators out;
- a new reference bus, the bus of the largest remaining generator, when the
  old one was split off or lost its last generator.

When a solve with generator Q limits doesn't converge, it runs again without
them, to say whether reactive limits are the cause.

## Ranking

A branch counts as overloaded when its MVA after the outage is above the
limit (100% of rate A by default) and it was under the limit before, or has
grown by more than one point. Pre-existing overloads that don't get worse
don't rank an outage. A bus counts when it is outside its voltage limits and
more than 0.001 p.u. further out than before.

Each outage's severity is its worst such overload in percent, or
$\text{limit} + 2000\,\Delta V$ for a voltage problem $\Delta V$ p.u.
outside, whichever is larger; failed AC solves rank first. Ties (for example
every outage of a case without ratings) are broken by the MW split off and
then by the largest flow change. The outages solved in AC are listed first,
then the linear-only ones.

The screen chooses which outages get an AC solve by the same severity, from
the linear results, so it checks the outages that cause new overloads first.
Voltage problems and reactive limits are only found by the AC solves.

## Checks

`crates/stannum-core/tests/contingency.rs` checks the linear trip of every
meshed branch of case30 against a DC power flow without it, each AC row
against solving the same outage as a study edit, islanding on case30, and
cancelling a run. `crates/rs-power/src/outage.rs` checks single trips against
the LODF matrix and double trips against two trips in sequence.
