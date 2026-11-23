import type { OidcClientMetadataJSON } from "./OidcClientMetadata.js";

export type OidcClientRegistrationResponse = OidcClientMetadataJSON & {
  client_id?: string;
  client_secret?: string;
  client_id_issued_at?: number;
  client_secret_expires_at?: number;
  registration_access_token?: string;
  registration_client_uri?: string;
  access_token_expiry?: number;
  refresh_token_expiry?: number;
  [key: string]: unknown;
};
