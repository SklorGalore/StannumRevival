import {afterEach,describe,expect,it,vi} from 'vitest';

const runtime=vi.hoisted(()=>({desktop:false,invoke:vi.fn()}));

vi.mock('@tauri-apps/api/core',()=>({
  isTauri:()=>runtime.desktop,
  invoke:runtime.invoke,
}));

import {saveExport} from './export';

afterEach(()=>{
  runtime.desktop=false;
  runtime.invoke.mockReset();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('saveExport',()=>{
  it('downloads the exact browser Blob with its MIME type and suggested filename',async()=>{
    const click=vi.fn();
    const anchor={href:'',download:'',click};
    const createObjectURL=vi.fn().mockReturnValue('blob:export');
    vi.stubGlobal('document',{createElement:vi.fn().mockReturnValue(anchor)});
    vi.stubGlobal('URL',{createObjectURL,revokeObjectURL:vi.fn()});

    await saveExport('bus,name\n1,North','text/csv;charset=utf-8','case-topology.csv');

    expect(createObjectURL).toHaveBeenCalledOnce();
    const blob=createObjectURL.mock.calls[0][0] as Blob;
    expect(blob.type).toBe('text/csv;charset=utf-8');
    await expect(blob.text()).resolves.toBe('bus,name\n1,North');
    expect(anchor).toMatchObject({href:'blob:export',download:'case-topology.csv'});
    expect(click).toHaveBeenCalledOnce();
  });

  it('revokes the browser download URL after the download delay',async()=>{
    vi.useFakeTimers();
    const revokeObjectURL=vi.fn();
    vi.stubGlobal('document',{createElement:vi.fn().mockReturnValue({click:vi.fn()})});
    vi.stubGlobal('URL',{createObjectURL:vi.fn().mockReturnValue('blob:export'),revokeObjectURL});

    await saveExport('{}','application/json','case-topology.json');
    expect(revokeObjectURL).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:export');
  });

  it('still schedules URL revocation when clicking the download link throws',async()=>{
    vi.useFakeTimers();
    const revokeObjectURL=vi.fn();
    vi.stubGlobal('document',{createElement:vi.fn().mockReturnValue({click:vi.fn(()=>{throw new Error('download blocked');})})});
    vi.stubGlobal('URL',{createObjectURL:vi.fn().mockReturnValue('blob:export'),revokeObjectURL});

    await expect(saveExport('contents','text/csv','case.csv')).rejects.toThrow('download blocked');
    await vi.advanceTimersByTimeAsync(1000);
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:export');
  });

  it('invokes the native save command with export text and suggested filename',async()=>{
    runtime.desktop=true;
    runtime.invoke.mockResolvedValue(undefined);

    await saveExport('{"ok":true}','application/json','case-topology.json');

    expect(runtime.invoke).toHaveBeenCalledWith('save_export',{
      data:'{"ok":true}',
      filename:'case-topology.json',
    });
  });

  it('treats a native dialog cancellation as a successful save result',async()=>{
    runtime.desktop=true;
    runtime.invoke.mockResolvedValue(undefined);

    await expect(saveExport('contents','text/csv','case.csv')).resolves.toBeUndefined();
  });

  it('propagates native save errors to the caller',async()=>{
    runtime.desktop=true;
    const failure=new Error('Could not write export file');
    runtime.invoke.mockRejectedValue(failure);

    await expect(saveExport('contents','text/csv','case.csv')).rejects.toBe(failure);
  });
});
