function generate_raw_reference(matpower_path, output_dir)
% Generate RAW conversion references using a trusted MATPOWER 8.1 checkout.
% Existing RAW input files are preserved; corresponding JSON is overwritten.
addpath(fullfile(matpower_path, 'lib'));
addpath(fullfile(matpower_path, 'mips', 'lib'));
addpath(fullfile(matpower_path, 'mp-opt-model', 'lib'));
addpath(fullfile(matpower_path, 'mptest', 'lib'));
assert(strcmp(mpver, '8.1'), 'MATPOWER 8.1 required');
files = dir(fullfile(output_dir, '*.raw'));
for k = 1:length(files)
    path = fullfile(output_dir, files(k).name);
    [mpc, warns] = psse2mpc(path, 0);
    expected = struct('matpower_version', mpver, 'bus', mpc.bus, ...
        'gen', mpc.gen, 'branch', mpc.branch, 'bus_name', {mpc.bus_name}, ...
        'baseMVA', mpc.baseMVA, 'warnings', {warns});
    if isfield(mpc, 'dcline')
        expected.dcline = mpc.dcline;
    end
    if ~isfield(mpc, 'dcline') && sum(mpc.bus(:, 2) == 3) == 1
        opt = mpoption('verbose', 0, 'out.all', 0, 'exp.use_legacy_core', 1, 'pf.tol', 1e-10, 'pf.nr.max_it', 30);
        for model = {'ac', 'dc'}
            opt = mpoption(opt, 'model', upper(model{1}));
            result = runpf(mpc, opt);
            expected.(model{1}) = struct('success', result.success, 'bus', result.bus, 'gen', result.gen, 'branch', result.branch);
        end
    end
    [~, name] = fileparts(path);
    fid = fopen(fullfile(output_dir, [name '.json']), 'w');
    assert(fid >= 0, 'Cannot open reference output');
    cleanup = onCleanup(@() fclose(fid));
    fprintf(fid, '%s\n', jsonencode(expected));
    clear cleanup;
end
end
