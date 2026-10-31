import type { Rating } from "./commands.gen";

export type { Rating };

/** The three branch ratings, in the order a planner lists them. */
export const RATINGS: { value: Rating; label: string; hint: string }[] = [
  { value: "a", label: "Rate A", hint: "normal" },
  { value: "b", label: "Rate B", hint: "short-term" },
  { value: "c", label: "Rate C", hint: "emergency" },
];

/** "rate A", for sentences. */
export function ratingLabel(r: Rating | null | undefined): string {
  return `rate ${(r ?? "a").toUpperCase()}`;
}
