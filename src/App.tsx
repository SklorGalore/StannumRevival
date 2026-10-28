import { Shell } from "./shell/Shell";
import { CrashBoundary } from "./shell/CrashNotice";

export default function App() {
  return (
    <CrashBoundary name="the window" window>
      <Shell />
    </CrashBoundary>
  );
}
