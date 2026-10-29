// Rows read from the core a block at a time, for tables too large to send
// whole (see `paging.rs` in stannum-core). The core sorts, searches and
// filters; the table asks only for the blocks in view.
import { useEffect, useRef, useState } from "react";

/** What a table asks its source for. */
export interface PageRequest {
  offset: number;
  limit: number;
  sort: { field: string; descending: boolean } | null;
  search: string;
  /** An element whose position among the matched rows comes back as `found`. */
  find: string | null;
}

/** One block of rows, and where it sits. */
export interface RowPage<T> {
  /** Rows passing the search and filters. */
  matched: number;
  /** Rows in the whole table. */
  total: number;
  offset: number;
  rows: T[];
  found: number | null;
}

/**
 * Where a paged table's rows come from. `key` names everything behind the
 * rows other than the sort and search (the result, the filters): when it
 * changes, the rows are read again.
 */
export interface RowSource<T> {
  key: string;
  fetch: (request: PageRequest) => Promise<RowPage<T>>;
}

/** Rows per request. */
export const BLOCK = 100;

export interface PagedRows<T> {
  /** Matched rows, or null before the first block arrives. */
  matched: number | null;
  total: number | null;
  /** The row at a position, once its block has arrived. */
  rowAt: (i: number) => T | undefined;
  /** Where the found element is, and a counter that changes each time it's looked up. */
  found: { index: number | null; n: number };
  error: string | null;
}

/**
 * The rows of `source` in `[first, last)`, fetched in blocks. A new key,
 * sort or search starts over; `find` is looked up again with it, and when
 * it changes.
 */
export function usePagedRows<T>(
  source: RowSource<T> | undefined,
  sort: PageRequest["sort"],
  search: string,
  first: number,
  last: number,
  find: string | null,
): PagedRows<T> {
  const query = source ? `${source.key}\u0000${JSON.stringify(sort)}\u0000${search}` : "";
  const blocks = useRef(new Map<number, T[]>());
  const asked = useRef(new Set<number>());
  const current = useRef(query);
  const [, redraw] = useState(0);
  const [counts, setCounts] = useState<{ matched: number | null; total: number | null }>({ matched: null, total: null });
  const [found, setFound] = useState<{ index: number | null; n: number }>({ index: null, n: 0 });
  const [error, setError] = useState<string | null>(null);

  // A new query drops every block.
  if (current.current !== query) {
    current.current = query;
    blocks.current = new Map();
    asked.current = new Set();
  }

  const fetchBlock = (block: number, withFind: string | null) => {
    if (!source) return;
    const q = query;
    asked.current.add(block);
    source
      .fetch({ offset: block * BLOCK, limit: BLOCK, sort, search, find: withFind })
      .then((page) => {
        if (current.current !== q) return;
        blocks.current.set(block, page.rows);
        setCounts((c) => (c.matched === page.matched && c.total === page.total ? c : { matched: page.matched, total: page.total }));
        if (withFind !== null) setFound((f) => ({ index: page.found, n: f.n + 1 }));
        setError(null);
        redraw((n) => n + 1);
      })
      .catch((e: Error) => {
        if (current.current !== q) return;
        asked.current.delete(block);
        setError(e.message);
      });
  };

  // The first block of a query carries the lookup, as does a new `find`.
  const lookedUp = useRef<{ query: string; find: string | null } | null>(null);
  useEffect(() => {
    if (!source) return;
    const fresh = lookedUp.current?.query !== query;
    if (fresh) setCounts({ matched: null, total: null });
    if (fresh || lookedUp.current?.find !== find) {
      lookedUp.current = { query, find };
      // Block 0 always comes first, so the count is known.
      if (fresh || find !== null) fetchBlock(0, find);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, find]);

  // The blocks in view.
  useEffect(() => {
    if (!source || counts.matched === null) return;
    const end = Math.min(last, counts.matched);
    for (let b = Math.floor(first / BLOCK); b * BLOCK < end; b++) {
      if (!asked.current.has(b)) fetchBlock(b, null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, first, last, counts.matched]);

  return {
    matched: counts.matched,
    total: counts.total,
    rowAt: (i) => blocks.current.get(Math.floor(i / BLOCK))?.[i % BLOCK],
    found,
    error,
  };
}

/** Every matched row of a source, for export: read in large pages. */
export async function allRows<T>(source: RowSource<T>, sort: PageRequest["sort"], search: string): Promise<T[]> {
  const out: T[] = [];
  const limit = 5000;
  for (let offset = 0; ; offset += limit) {
    const page = await source.fetch({ offset, limit, sort, search, find: null });
    out.push(...page.rows);
    if (page.rows.length < limit || out.length >= page.matched) return out;
  }
}

const objectKeys = new WeakMap<object, number>();
let nextObjectKey = 1;

/** A number for an object, the same each time: part of a source's key, so it changes with a new result. */
export function objectKey(o: object): number {
  let key = objectKeys.get(o);
  if (key === undefined) objectKeys.set(o, (key = nextObjectKey++));
  return key;
}
