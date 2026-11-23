export function generateState(): string {
  try {
    const arr = new Uint8Array(16);
    if (typeof crypto !== "undefined" && typeof crypto.getRandomValues === "function") {
      crypto.getRandomValues(arr);
    } else {
      for (let i = 0; i < arr.length; i++) {
        arr[i] = Math.floor(Math.random() * 256);
      }
    }
    let state = "";
    for (const byte of arr) {
      state += byte.toString(16).padStart(2, "0");
    }
    return state;
  } catch {
    return Math.random().toString(36).substring(2);
  }
}
