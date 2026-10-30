/**
 * The version as the module rail shows it: major.minor, plus any pre-release
 * suffix so a release candidate never passes for the release
 * ("1.0.0" -> "v1.0", "1.0.0-rc.1" -> "v1.0-rc.1").
 */
export function shortVersion(version: string): string {
  const [core, ...pre] = version.split("+")[0].split("-");
  const label = `v${core.split(".").slice(0, 2).join(".")}`;
  return pre.length ? `${label}-${pre.join("-")}` : label;
}
