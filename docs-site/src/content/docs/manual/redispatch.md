---
title: Redispatch (DC OPF)
description: How Stannum relieves branch overloads by moving generation: a linear program on DC distribution factors, with AC passes that correct the estimate.
---

Redispatch starts from the last converged AC solve and moves generator
outputs to bring monitored branches within a limit. It is a DC optimal
power flow with enforced branch limits, whose objective is the total
movement rather than cost: RAW cases carry no cost data, and a planner
relieving an overload usually wants the smallest change that does it.

## The linear program

For each unit that may move, two variables, `up` and `down`, each between
zero and the room to Pmax or Pmin. The program minimizes

```text
sum(up + down) + 1000 · sum(over)
```

subject to, for each island, `sum(up − down) = 0`, and for each branch
constraint `l`

```text
| f0[l] + sum over units g of s[l][g] · (up[g] − down[g]) | ≤ cap[l] + over[l]
```

where `f0` is the AC active flow at the from end, `s[l][g]` is the DC
distribution factor (PTDF) of branch `l` for an injection at unit `g`'s bus
taken out at the island's reference bus, and `over` is an overload slack
that only becomes non-zero when no move can bring the branch back. With the
balance constraint the choice of reference bus cancels out.

Only branches that are, or would become, over their cap are constraints. The
program starts with the ones over now, solves, checks every monitored branch
with one DC solve of the moves, adds any pushed over, and solves again. One
PTDF row costs one sparse solve with the factorized susceptance matrix (it is
symmetric, so a branch's row is a solve with that branch's two ends).

The program is solved with a simplex method (the `microlp` crate), so the
answer is a corner of the feasible region: with one binding branch, exactly
two units move, the pair whose distribution factors differ most for the room
they have. A test checks this against every pair on case9.

## Caps from MVA ratings

Ratings are in MVA and the program works in MW. A branch's cap is the active
flow that fits beside its present reactive flow at the end that loads it
most,

```text
cap = sqrt((rating · limit · 0.998)² − Q²) + (|P_from| − |P_to|)
```

with the second term moving it to the from end when the far end is the more
loaded one. The 0.998 aims just inside the limit.

## AC passes

After each program the case is solved in AC with the new outputs (from the
last solution's voltages). Losses, reactive flow and controls move AC
loadings away from the DC estimate, so a branch still over has its cap scaled
by `limit / loading` and the next pass moves units again from where the last
left them, up to four passes. The reference generator takes up the change in
losses. A pass whose AC solve fails is undone and the result says so.

## What it leaves out

- Generator costs: the objective is total movement.
- Post-contingency limits (preventive security constraints): redispatch
  enforces limits in the case as it is. N-1-1 studies use it after the first
  outage.
- Phase shifters, HVDC and load as controls: only generator outputs move.
- The attached VSC DC grids.
