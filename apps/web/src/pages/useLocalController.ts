import { useEffect, useState } from "preact/hooks";
import type { LocalController } from "../services/local-session";
import { useUi } from "../ui/context";
export function useLocalController<View>(controller: LocalController<View>) {
  const { services } = useUi();
  const [snapshot, setSnapshot] = useState({
    controller,
    state: controller.read(),
  });
  useEffect(() => {
    const unsubscribe = controller.subscribe(() =>
      setSnapshot({ controller, state: controller.read() }),
    );
    setSnapshot({ controller, state: controller.read() });
    let release: (() => void) | undefined;
    void controller.start(() => {
      release = services.activity.hold();
    });
    return () => {
      unsubscribe();
      controller.dispose();
      release?.();
    };
  }, [controller, services]);
  return snapshot.controller === controller
    ? snapshot.state
    : controller.read();
}
