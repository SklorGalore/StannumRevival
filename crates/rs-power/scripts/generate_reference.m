function generate_reference(matpower_root, output_root)
% Regenerate checked-in fixtures, using a trusted MATPOWER 8.1 checkout.
% Example: octave --no-gui --quiet --eval "addpath('scripts'); generate_reference('.reference', 'tests/fixtures')"
    addpath(genpath(matpower_root));
    assert(strcmp(mpver(), '8.1'), 'Reference generation requires MATPOWER 8.1');
    if ~exist(output_root, 'dir'), mkdir(output_root); end
    options = mpoption('verbose', 0, 'out.all', 0, ...
        'exp.use_legacy_core', 1, 'pf.alg', 'NR', ...
        'pf.tol', 1e-10, 'pf.nr.max_it', 10);
    names = {'case9', 'case14', 'case30', 'case57', 'case118', 'case300'};
    for k = 1:numel(names)
        name = names{k};
        copyfile(fullfile(matpower_root, 'data', [name '.m']), fullfile(output_root, [name '.m']));
        emit(name, loadcase(name), options, output_root, false);
    end
    % Synthetic modifications isolate compatibility behavior.
    c = loadcase('case9'); c.gen(2, 4) = 5;
    emit('q_limit', c, options, output_root, true);
    c = loadcase('case9'); c.gen(1, 4) = 5;
    emit('slack_limit', c, options, output_root, true);
    c = loadcase('case9'); c.gen(:, 4) = -1000;
    c.gen(:, 5) = -2000;
    emit('q_infeasible', c, options, output_root, true);
    c = loadcase('case9');
    c.gen(4, :) = c.gen(2, :); c.gen(2, 2:3) = c.gen(2, 2:3)/2;
    c.gen(4, 2:3) = c.gen(2, 2:3); c.gen(4, 4:5) = [100, -50];
    emit('shared_gen', c, options, output_root, false);
    c.gen([2,4],4:5) = repmat([Inf,-Inf],2,1);
    emit('infinite_q', c, options, output_root, false);
    c.gen([2,4],4:5) = 0;
    emit('zero_q_range', c, options, output_root, false);
    c = loadcase('case9'); c.branch(2,9:10) = [1.05, 7];
    c.bus(5,5:6) = [2,3];
    c.branch(end+1,:) = c.branch(2,:);
    c.branch(end+1,:) = c.branch(3,:); c.branch(end,11) = 0;
    c.gen(end+1,:) = c.gen(2,:); c.gen(end,8)=0;
    c.bus(:,1)=c.bus(:,1)*10+7; c.gen(:,1)=c.gen(:,1)*10+7;
    c.branch(:,1:2)=c.branch(:,1:2)*10+7;
    emit('equipment', c, options, output_root, false);
    copyfile(fullfile(matpower_root, 'LICENSE'), fullfile(output_root, 'MATPOWER-LICENSE'));
    copyfile(fullfile(matpower_root, 'AUTHORS'), fullfile(output_root, 'MATPOWER-AUTHORS'));
end

function emit(name, c, options, output_root, q_limits)
    if isfield(c, 'gencost') && size(c.gencost,1) ~= size(c.gen,1)
        c = rmfield(c, 'gencost');
    end
    opts = mpoption(options, 'pf.enforce_q_lims', double(q_limits));
    ac = runpf(c, opts);
    dc = rundcpf(c, options);
    record = struct('matpower_version', mpver(), 'legacy_core', true, ...
        'enforce_q_limits', q_limits, 'input', c, ...
        'ac', compact(ac), 'dc', compact(dc));
    fid = fopen(fullfile(output_root, [name '.json']), 'w');
    assert(fid >= 0, 'Cannot open fixture');
    cleanup = onCleanup(@() fclose(fid));
    fprintf(fid, '%s\n', jsonencode(record));
end

function r = compact(c)
    r = struct('success', logical(c.success), 'bus', c.bus, ...
        'gen', c.gen, 'branch', c.branch, 'iterations', c.iterations);
end
