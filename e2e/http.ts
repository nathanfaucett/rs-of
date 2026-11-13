export async function checkedFetch(
  input: RequestInfo | URL,
  init?: RequestInit
): Promise<Response> {
  const response = await fetch(input, init);
  if (!response.ok) {
    const method = init?.method ?? (input instanceof Request ? input.method : "GET");
    throw new Error(`${method} ${response.url}: ${response.status} ${await response.text()}`);
  }
  return response;
}
