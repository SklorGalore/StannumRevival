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

**Shunt and DC line outages** (planning events only) are injection changes
in the same way. A shunt gives back its conductance, $G\,V^2$ MW at the base
voltage $V$; its reactive power doesn't move MW in the linear model. A DC
line keeping a share $k$ of its scheduled flow $P$ (0 when it goes out, 0.5
for one pole) moves its sending bus by $(1-k)P$ and its receiving bus by the
change in what arrives, $P_r(kP) - P_r(P)$ with
$P_r(p) = p - L_0 - L_1 p$ and the line's loss terms $L_0$, $L_1$. The
base solution's DC flow is used.

## AC check

The worst screened outages (or all of them) are solved with the full AC
Newton power flow and the base case's solve settings. Each starts from the
base solution, as in [Starting
point](/StannumRevival/manual/ac-power-flow/#starting-point), with the outage applied:

- tripped branches and generators out of service;
- a tripped shunt's conductance and susceptance taken off its bus; a
  switched shunt in voltage control comes off at the step the base solution
  left it on, with no blocks left to switch;
- a tripped DC line out of service, or for one pole its scheduled power (or
  the detailed model's power or current order) halved;
- split-off buses made isolated, with their generators out;
- a new reference bus, the bus of the largest remaining generator, when the
  old one was split off or lost its last generator.

When a solve with generator Q limits doesn't converge, it runs again without
them, to say whether reactive limits are the cause.

## N-2 and bus outages

N-2 runs every pair of the chosen branches and generators: $n$ elements
make $n(n-1)/2$ outages, at most 250,000. Each pair is screened like an N-1
outage with the linear factors of both elements out together, and the worst
are solved in AC. Listing ids joined by `&` (for example
`line:1:2:1&generator:22:1`) runs exactly those elements together, from run
files, Python or Trip and compare.

A bus outage opens every branch and DC line at the bus. The bus is then
cut off, and its load, generation and shunts count as lost, the same as any
other outage that splits the network.

## Planning events

`crates/stannum-core/src/planning.rs` generates NERC TPL-001 Table 1 events
from the case. The elements are those already listed above, so each event is
one outage of the run:

- **P0**: no outage. It is ranked like the others, but judged against the
  limits instead of against itself: every branch over the limit and every
  bus outside its limits counts.
- **P1.1 to P1.3**: each in-service generator, line and transformer. Lines
  in a RAW multi-section line grouping (`I, J, 'ID', MET, DUM1, ...`) are
  left out on their own and taken out together as `line:<I>:<J>:<ID>`, as
  the breakers at its two ends would take it out.
- **P1.4**: each in-service fixed shunt with a nonzero G or B, and each
  in-service switched shunt with a nonzero BINIT or in voltage control. A
  MATPOWER bus's GS and BS count as one fixed shunt.
- **P1.5**: each in-service two-terminal (line-commutated) DC line with half
  its schedule. RAW has one record for both poles of a bipole, so this
  assumes two equal poles. VSC lines get no P1.5 event.
- **P2.1**: each section of a multi-section line on its own, as opening it
  without a fault would.
- **P7.2**: each in-service two-terminal and VSC DC line. Multi-terminal
  systems are left out.

Filters: a branch, shunt or DC line counts when any terminal bus is at or
above the voltage (buses with no base voltage count), and when any terminal
bus is in the chosen areas. Generators only take the area filter.

## Ranking

A branch counts as overloaded when its MVA after the outage is above the
limit (100% by default) of the contingency rating, and it was under the
limit of the base case rating before, or has grown by more than one point.
Both ratings are rate A unless chosen otherwise; a common planning choice
is rate A before and rate B after. A branch whose chosen rating is zero is
unrated and never overloads. The base case event (P0) is held to the base
case rating. Pre-existing overloads that don't get worse
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
In a planning event run, P0 is always solved, and shunt outages, which the
screen can't judge, get a separate budget of the same size, largest shunt
(by $\sqrt{G^2 + B^2}$) first.
Voltage problems and reactive limits are only found by the AC solves.

## Checks

`crates/stannum-core/tests/planning_events.rs` checks each category's
events on a five-bus case with shunts, a DC line and a multi-section line;
shunt and DC line events against solving a case without the shunt, with the
DC schedule halved or with the DC line blocked; the linear screen of a DC
line outage against its AC solve; and P0's base-case overloads.
`crates/stannum-core/tests/contingency.rs` checks the linear trip of every
meshed branch of case30 against a DC power flow without it, each AC row
against solving the same outage as a study edit, islanding on case30, and
cancelling a run. `crates/rs-power/src/outage.rs` checks single trips against
the LODF matrix and double trips against two trips in sequence.
