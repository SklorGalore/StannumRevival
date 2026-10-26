% MATACDC_BENCHMARK  Solve Stannum's VSC grid benchmark cases with MatACDC.
%
%   Cross-checks Stannum's VSC grid solver against MatACDC
%   (www.esat.kuleuven.be/electa/teaching/matacdc/, J. Beerten, KU Leuven).
%   Converts Stannum's own DC grid files to MatACDC's input, solves them,
%   and writes the results to
%   crates/stannum-core/tests/fixtures/vsc_grids/benchmark/matacdc_results.json,
%   which tests/matacdc_benchmark.rs compares against.
%
%   MatACDC is not part of this repository and may not be redistributed.
%   Download MatACDC 1.0 from its website yourself, then run from the
%   repository root with GNU Octave 7+ and MATPOWER 7+:
%
%     MATPOWER_PATH=<matpower folder> MATACDC_PATH=<MatACDC1.0 folder> \
%       octave --no-gui scripts/matacdc_benchmark.m
%
%   MatACDC is a cross-check only: Stannum's model follows the published
%   paper, and differences are documented, not tuned away.

1; % script file

function main()
  root = fileparts(fileparts(mfilename('fullpath')));
  dir = fullfile(root, 'crates', 'stannum-core', 'tests', 'fixtures', 'vsc_grids');
  mp = getenv('MATPOWER_PATH');
  acdc = getenv('MATACDC_PATH');
  if isempty(mp) || isempty(acdc)
    error('set MATPOWER_PATH and MATACDC_PATH');
  end
  addpath(fullfile(mp, 'lib'), fullfile(mp, 'mptest', 'lib'), ...
          fullfile(mp, 'mips', 'lib'), fullfile(mp, 'mp-opt-model', 'lib'));
  addpath(acdc);

  ac = loadcase(fullfile(dir, 'two_area.m'));
  % MatACDC wants one AC reference bus per zone: each island is its own zone
  % (bus numbers 1xx, 2xx, 3xx).
  ac.bus(:, 11) = floor(ac.bus(:, 1) / 100);
  ac.gen(:, end+1:21) = 0;

  % name, DC grid file, enforce converter limits
  scenarios = {
    'base',   fullfile(dir, 'two_area.dcgrid.json'), 0;
    'a2_out', fullfile(dir, 'benchmark', 'two_area_a2_out.dcgrid.json'), 0;
    'limits', fullfile(dir, 'benchmark', 'two_area_limits.dcgrid.json'), 1;
  };
  out = struct();
  out.source = 'MatACDC 1.0 with MATPOWER, scripts/matacdc_benchmark.m';
  for k = 1:rows(scenarios)
    dc = to_matacdc(ac, jsondecode(fileread(scenarios{k, 2}), 'makeValidName', false));
    out.(scenarios{k, 1}) = solve(ac, dc, scenarios{k, 3});
  end
  file = fullfile(dir, 'benchmark', 'matacdc_results.json');
  fid = fopen(file, 'w');
  fprintf(fid, '%s\n', jsonencode(out));
  fclose(fid);
  printf('wrote %s\n', file);
end

function v = field(s, name, default)
  if isstruct(s) && isfield(s, name)
    v = s.(name);
  else
    v = default;
  end
end

function c = as_cell(x)
  if iscell(x)
    c = x;
  else
    c = num2cell(x);
  end
end

function dc = to_matacdc(ac, file)
  % Stannum's DC grid file in MatACDC's matrices. All grids must share one
  % pole count, which MatACDC takes per case.
  S = ac.baseMVA;
  dc.baseMVAac = S;
  dc.baseMVAdc = S;
  busdc = []; convdc = []; branchdc = [];
  grids = as_cell(file.grids);
  dc.pol = grids{1}.poles;
  for g = 1:numel(grids)
    G = grids{g};
    if G.poles ~= dc.pol
      error('MatACDC needs one pole count for all grids');
    end
    kv = G.base_kv;
    convs = as_cell(G.converters);
    buses = as_cell(G.buses);
    for b = 1:numel(buses)
      acbus = 0; vdc = 1;
      for k = 1:numel(convs)
        if convs{k}.dc_bus == buses{b}.number
          acbus = convs{k}.ac_bus;
          vdc = field(convs{k}.dc_control, 'v_dc_pu', 1);
        end
      end
      busdc(end+1, :) = [buses{b}.number, acbus, g, 0, vdc, kv, 1.1, 0.9, 0];
    end
    for k = 1:numel(convs)
      c = convs{k};
      mode = c.dc_control.mode;
      tdc = find(strcmp(mode, {'power', 'slack', 'droop'}));
      tac = 1 + strcmp(c.ac_control.mode, 'voltage');
      tf = field(c, 'transformer', struct());
      re = field(c, 'reactor', struct());
      L = c.losses;
      lim = field(c, 'limits', struct());
      kvac = ac.bus(ac.bus(:, 1) == c.ac_bus, 10);
      imax = field(lim, 'i_max_pu', 0);    if imax == 0, imax = 10; end
      ucmax = field(lim, 'uc_max_pu', 0);  if ucmax == 0, ucmax = 1.5; end
      ucmin = field(lim, 'uc_min_pu', 0);  if ucmin == 0, ucmin = 0.5; end
      p = field(c.dc_control, 'p_mw', 0);
      if tdc == 3
        % Stannum: power out of the DC grid = p + k (U - Uset), k in MW/kV.
        droop = [1 / (c.dc_control.mw_per_kv * kv), p, c.dc_control.v_dc_pu, 0];
      else
        droop = [0 0 0 0];
      end
      % Stannum's c_inverter applies while the converter delivers power to
      % the AC grid, which is MatACDC's LossCrec column.
      convdc(end+1, :) = [c.dc_bus, tdc, tac, p, field(c.ac_control, 'q_mvar', 0), ...
        field(c.ac_control, 'v_pu', 1), field(tf, 'r_pu', 0), field(tf, 'x_pu', 0), ...
        field(c, 'filter_b_pu', 0), field(re, 'r_pu', 0), field(re, 'x_pu', 0), kvac, ...
        ucmax, ucmin, imax, field(c, 'in_service', true), ...
        field(L, 'a_mw', 0), field(L, 'b_kv', 0), field(L, 'c_inverter_ohm', 0), ...
        field(L, 'c_rectifier_ohm', 0), droop];
    end
    lines = as_cell(field(G, 'lines', {}));
    for l = 1:numel(lines)
      Ln = lines{l};
      branchdc(end+1, :) = [Ln.from, Ln.to, Ln.r_ohm * S / kv^2, 0, 0, 100, 100, 100, ...
        field(Ln, 'in_service', true)];
    end
  end
  dc.busdc = busdc; dc.convdc = convdc; dc.branchdc = branchdc;
end

function r = solve(ac, dc, limits)
  mpopt = mpoption('verbose', 0, 'out.all', 0, 'pf.tol', 1e-10);
  opt = macdcoption;
  opt(10) = limits;  % converter limits
  opt(13) = 0;       % no printed output
  [~, bus, ~, ~, busdc, convdc, branchdc, converged] = runacdcpf(ac, dc, opt, mpopt);
  if ~converged
    error('MatACDC did not converge');
  end
  r.converters = struct('dc_bus', num2cell(convdc(:, 1)), ...
    'ps_mw', num2cell(convdc(:, 4)), 'qs_mvar', num2cell(convdc(:, 5)), ...
    'uc_pu', num2cell(convdc(:, 25)), 'uc_deg', num2cell(convdc(:, 26)), ...
    'pc_mw', num2cell(convdc(:, 27)), 'qc_mvar', num2cell(convdc(:, 28)), ...
    'loss_mw', num2cell(convdc(:, 29)));
  r.dc_buses = struct('number', num2cell(busdc(:, 1)), 'v_pu', num2cell(busdc(:, 5)));
  r.dc_lines = struct('from', num2cell(branchdc(:, 1)), 'to', num2cell(branchdc(:, 2)), ...
    'from_mw', num2cell(branchdc(:, 10)), 'to_mw', num2cell(branchdc(:, 11)));
  r.ac_buses = struct('id', num2cell(bus(:, 1)), 'vm', num2cell(bus(:, 8)), ...
    'va_deg', num2cell(bus(:, 9)));
end

main();
