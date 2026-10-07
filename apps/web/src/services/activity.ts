export interface ActivityPort {
  hold(): () => void;
  read(): { busy: boolean; locked: boolean };
  subscribe(listener: () => void): () => void;
  prepare(token: string): boolean;
  release(token: string): void;
}
export function createActivity(): ActivityPort {
  let holds = 0;
  let token: string | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const listeners = new Set<() => void>();
  const emit = () => {
    for (const listener of listeners) listener();
  };
  const release = (value: string) => {
    if (token !== value) return;
    token = undefined;
    clearTimeout(timer);
    timer = undefined;
    emit();
  };
  return {
    hold() {
      if (token) throw Error("updating");
      let active = true;
      holds++;
      emit();
      return () => {
        if (active) {
          active = false;
          holds--;
          emit();
        }
      };
    },
    read: () => ({ busy: holds > 0, locked: token !== undefined }),
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    prepare(value) {
      if (
        !value ||
        value.length > 128 ||
        holds > 0 ||
        (token && token !== value)
      )
        return false;
      if (!token) {
        token = value;
        timer = setTimeout(() => release(value), 15000);
        emit();
      }
      return true;
    },
    release,
  };
}
