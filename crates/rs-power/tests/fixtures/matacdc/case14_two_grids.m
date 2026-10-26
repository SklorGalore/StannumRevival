function [baseMVAac, baseMVAdc, pol, busdc, convdc, branchdc] = case14_two_grids
%CASE14_TWO_GRIDS  Two VSC DC grids on MATPOWER's case14, in MatACDC's format.
%   Written for Stannum's tests (not taken from MatACDC). Grid 1 is a meshed
%   five-bus grid with a DC slack, two droop converters (one with a
%   deadband), a power-controlled converter and a junction bus with no
%   converter. Grid 2 is a point-to-point link held by a droop converter.
%   Converter data follow the typical values of Beerten et al., IEEE TPWRS
%   27(2), 2012.

%% system MVA base
baseMVAac = 100;
baseMVAdc = 100;

%% dc grid topology
pol = 2;

%% dc bus data
%   busdc_i busac_i grid    Pdc     Vdc     basekVdc    Vdcmax  Vdcmin  Cdc
busdc = [
    1       2       1       0       1       320         1.1     0.9     0;
    2       5       1       0       1       320         1.1     0.9     0;
    3       9       1       0       1       320         1.1     0.9     0;
    4       13      1       0       1       320         1.1     0.9     0;
    5       0       1       0       1       320         1.1     0.9     0;
    6       11      2       0       1       150         1.1     0.9     0;
    7       14      2       0       1       150         1.1     0.9     0;
];

%% converters
%   busdc_i type_dc type_ac P_g     Q_g     Vtar    rtf     xtf     bf      rc      xc      basekVac Vmmax  Vmmin   Imax    status  LossA   LossB   LossCrec LossCinv droop   Pdcset  Vdcset  dVdcset
convdc = [
    1       3       1       0       10      1       0.0015  0.1121  0.0887  0.0001  0.16428 135     1.1     0.9     1.1     1       1.103   0.887   2.885   4.371   0.005   -20     1.0     0;
    2       2       2       0       0       1.02    0.0015  0.1121  0.0887  0.0001  0.16428 135     1.1     0.9     1.1     1       1.103   0.887   2.885   4.371   0       0       0       0;
    3       1       1       -40     5       1       0.0015  0.1121  0.0887  0.0001  0.16428 135     1.1     0.9     1.1     1       1.103   0.887   2.885   4.371   0       0       0       0;
    4       3       1       0       0       1       0.001   0.10    0       0.0001  0.16    135     1.1     0.9     1.1     1       1.103   0.887   2.885   4.371   0.004   25      1.0     0.002;
    6       3       1       0       -5      1       0       0       0       0.0001  0.16    135     1.1     0.9     1.1     1       1.103   0.887   2.885   4.371   0.01    -17     1.0     0;
    7       1       2       15      0       1.03    0.0015  0.1121  0.0887  0.0001  0.16428 135     1.1     0.9     1.1     1       1.103   0.887   2.885   4.371   0       0       0       0;
];

%% dc branches
%   fbusdc  tbusdc  r       l       c       rateA   rateB   rateC   status
branchdc = [
    1       2       0.052   0       0       100     100     100     1;
    2       3       0.052   0       0       100     100     100     1;
    3       4       0.073   0       0       100     100     100     1;
    4       1       0.073   0       0       100     100     100     1;
    2       5       0.03    0       0       100     100     100     1;
    5       4       0.03    0       0       100     100     100     1;
    6       7       0.06    0       0       100     100     100     1;
];
