import { useEffect, useState } from "preact/hooks";
export function useSnapshot<T>(port: {
  read(): T;
  subscribe(listener: () => void): () => void;
}): T {
  const [value, setValue] = useState(() => port.read());
  useEffect(() => {
    const unsubscribe = port.subscribe(() => setValue(port.read()));
    setValue(port.read());
    return unsubscribe;
  }, [port]);
  return value;
}
