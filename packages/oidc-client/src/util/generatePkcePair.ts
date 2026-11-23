const PKCE_VERIFIER_BYTES = 32;

type CryptoLike = {
  getRandomValues: (array: Uint8Array) => Uint8Array;
  subtle?: {
    digest: (algorithm: string, data: Uint8Array) => Promise<ArrayBuffer>;
  };
};

export type PkcePair = {
  codeVerifier: string;
  codeChallenge: string;
};

function getCrypto(): CryptoLike {
  const cryptoObject = (globalThis as { crypto?: CryptoLike }).crypto;
  if (!cryptoObject || typeof cryptoObject.getRandomValues !== "function") {
    throw new Error("Web Crypto API is required to generate PKCE values");
  }
  return cryptoObject;
}

function encodeBase64Url(data: Uint8Array): string {
  const bufferCtor = (
    globalThis as {
      Buffer?: {
        from(data: Uint8Array): { toString(encoding: string): string };
      };
    }
  ).Buffer;
  if (bufferCtor) {
    return bufferCtor
      .from(data)
      .toString("base64")
      .replace(/\+/g, "-")
      .replace(/\//g, "_")
      .replace(/=+$/g, "");
  }

  let binary = "";
  for (const byte of data) {
    binary += String.fromCharCode(byte);
  }
  if (typeof btoa !== "function") {
    throw new Error("No base64 encoder available for PKCE generation");
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

export async function generatePkcePair(): Promise<PkcePair> {
  const cryptoObject = getCrypto();
  const random = new Uint8Array(PKCE_VERIFIER_BYTES);
  cryptoObject.getRandomValues(random);
  const codeVerifier = encodeBase64Url(random);

  if (!cryptoObject.subtle) {
    throw new Error("SubtleCrypto API is required to generate PKCE values");
  }

  const digest = await cryptoObject.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(codeVerifier)
  );
  const codeChallenge = encodeBase64Url(new Uint8Array(digest));

  return {
    codeVerifier,
    codeChallenge,
  };
}
