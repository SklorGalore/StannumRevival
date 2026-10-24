import { afterEach, describe, expect, it } from 'vitest';
import { BrowserEngine } from './engine';
import type { Snapshot, WorkerRequest, WorkerResponse } from './types';

class FakeWorker {
  onmessage: ((event: MessageEvent<WorkerResponse>) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  requests: WorkerRequest[] = [];
  terminated = false;
  postMessage(request: WorkerRequest) { this.requests.push(request); }
  terminate() { this.terminated = true; }
  respond(response: WorkerResponse) { this.onmessage?.({ data: response } as MessageEvent<WorkerResponse>); }
}

const snapshot = {} as Snapshot;
let workers: FakeWorker[] = [];
const originalWorker = globalThis.Worker;

afterEach(() => {
  globalThis.Worker = originalWorker;
  workers = [];
});

function useFakeWorker() {
  globalThis.Worker = class extends FakeWorker {
    constructor(..._args: ConstructorParameters<typeof Worker>) { super(); workers.push(this); }
  } as unknown as typeof Worker;
  return workers;
}

describe('BrowserEngine topology requests', () => {
  it('sends the selected merge mode with load and resolves its snapshot', async () => {
    const created = useFakeWorker();
    const engine = new BrowserEngine();
    const bytes = new ArrayBuffer(4);
    const result = engine.load(bytes, true);
    const request = created[0].requests[0];
    expect(request).toMatchObject({ type: 'load', bytes, mergeZeroImpedance: true });
    created[0].respond({ id: request.id, type: 'loaded', snapshot });
    await expect(result).resolves.toBe(snapshot);
    engine.dispose();
  });

  it('sends topology changes to the retained worker and resolves the rebuilt snapshot', async () => {
    const created = useFakeWorker();
    const engine = new BrowserEngine();
    const first = engine.load(new ArrayBuffer(1));
    const load = created[0].requests[0];
    created[0].respond({ id: load.id, type: 'loaded', snapshot });
    await first;
    const changed = engine.setMergeZeroImpedance(true);
    expect(created[0].requests).toHaveLength(2);
    const request = created[0].requests[1];
    expect(request).toMatchObject({ type: 'topology', mergeZeroImpedance: true });
    created[0].respond({ id: request.id, type: 'loaded', snapshot });
    await expect(changed).resolves.toBe(snapshot);
    engine.dispose();
  });

  it('rejects only the failed request while another pending request can resolve', async () => {
    const created = useFakeWorker();
    const engine = new BrowserEngine();
    const first = engine.load(new ArrayBuffer(1));
    const load = created[0].requests[0];
    created[0].respond({ id: load.id, type: 'loaded', snapshot });
    await first;
    const failing = engine.setMergeZeroImpedance(true);
    const succeeding = engine.setMergeZeroImpedance(false);
    const [failedRequest, succeedingRequest] = created[0].requests.slice(1);
    created[0].respond({ id: failedRequest.id, type: 'error', error: { severity: 'error', section: 'APPLICATION', line: 0, message: 'rebuild failed' } });
    await expect(failing).rejects.toMatchObject({ message: 'rebuild failed' });
    created[0].respond({ id: succeedingRequest.id, type: 'loaded', snapshot });
    await expect(succeeding).resolves.toBe(snapshot);
    engine.dispose();
  });
});
