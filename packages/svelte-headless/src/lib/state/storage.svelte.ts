import { BROWSER } from "esm-env";

interface Serializer<T> {
  parse(text: string): T;
  stringify(object: T): string;
}

type StorageType = "local" | "session";

interface StorageOptions<T> {
  storage?: StorageType;
  serializer?: Serializer<T>;
  syncTabs?: boolean;
  onWriteError?(error: unknown): void;
  onParseError?(error: unknown): void;
  beforeRead?(value: T): T;
  beforeWrite?(value: T): T;
}

export function createStorage<T>(key: string, initialValue: T, options: StorageOptions<T> = {}) {
  const {
    storage = "local",
    serializer = JSON,
    syncTabs = true,
    onWriteError = console.error,
    onParseError = console.error,
    beforeRead = (v: T) => v,
    beforeWrite = (v: T) => v,
  } = options;

  const storageArea = BROWSER ? (storage === "local" ? localStorage : sessionStorage) : null;

  let storedValue: T;

  try {
    const item = storageArea?.getItem(key);
    storedValue = item ? beforeRead(serializer.parse(item)) : initialValue;
  } catch (error) {
    onParseError(error);
    storedValue = initialValue;
  }

  let state = $state(storedValue);

  function updateStorage(value: T) {
    try {
      const valueToStore = beforeWrite(value);
      storageArea?.setItem(key, serializer.stringify(valueToStore));
    } catch (error) {
      onWriteError(error);
    }
  }

  if (syncTabs && BROWSER && storage === "local") {
    window.addEventListener("storage", (event) => {
      if (event.key === key && event.storageArea === localStorage) {
        try {
          const newValue = event.newValue ? serializer.parse(event.newValue) : initialValue;
          state = beforeRead(newValue);
        } catch (error) {
          onParseError(error);
        }
      }
    });
  }

  $effect.root(() => {
    $effect(() => {
      updateStorage(state);
    });

    return () => {};
  });

  return {
    get item() {
      return state;
    },
    set item(newValue: T) {
      state = newValue;
    },
    update(updater: (value: T) => T) {
      state = updater(state);
    },
    reset() {
      state = initialValue;
    },
  };
}
