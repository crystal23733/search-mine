import { useEffect, useState } from "preact/hooks";
import type { LocalController } from "../services/local-session";
export function useLocalController<View>(controller: LocalController<View>) {
  const [snapshot, setSnapshot] = useState({
    controller,
    state: controller.read(),
  });
  useEffect(() => {
    const unsubscribe = controller.subscribe(() =>
      setSnapshot({ controller, state: controller.read() }),
    );
    setSnapshot({ controller, state: controller.read() });
    void controller.start();
    return () => {
      unsubscribe();
      controller.dispose();
    };
  }, [controller]);
  return snapshot.controller === controller
    ? snapshot.state
    : controller.read();
}
