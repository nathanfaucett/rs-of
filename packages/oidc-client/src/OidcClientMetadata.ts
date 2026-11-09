export type JsonWebKey = {
  kty: string;
  use?: "sig" | "enc" | string;
  key_ops?: Array<
    | "sign"
    | "verify"
    | "encrypt"
    | "decrypt"
    | "wrapKey"
    | "unwrapKey"
    | "deriveKey"
    | "deriveBits"
    | string
  >;
  alg?: string;
  kid?: string;
  x5u?: string;
  x5c?: string[];
  x5t?: string;
  "x5t#S256"?: string;
  crv?: string;
  x?: string;
  y?: string;
  d?: string;
  n?: string;
  e?: string;
  p?: string;
  q?: string;
  dp?: string;
  dq?: string;
  qi?: string;
  k?: string;
};

export type JsonWebKeySet = {
  keys: JsonWebKey[];
};

export type OidcClientMetadata = {
  redirectUris: string[];
  application?: { uri?: string };
  tokenEndpointAuthMethod?:
    | "client_secret_basic"
    | "client_secret_post"
    | "client_secret_jwt"
    | "private_key_jwt"
    | "none"
    | string;
  grantTypes?: string[];
  responseTypes?: string[];
  clientName?: string;
  clientUri?: string;
  logoUri?: string;
  scope?: string;
  contacts?: string[];
  tosUri?: string;
  policyUri?: string;
  jwksUri?: string;
  jwks?: JsonWebKeySet;
  softwareId?: string;
  softwareVersion?: string;
  sectorIdentifierUri?: string;
  subjectType?: "public" | "pairwise";
  idTokenSignedResponseAlg?: string;
  idTokenEncryptedResponseAlg?: string;
  idTokenEncryptedResponseEnc?: string;
  userinfoSignedResponseAlg?: string;
  userinfoEncryptedResponseAlg?: string;
  userinfoEncryptedResponseEnc?: string;
  requestObjectSigningAlg?: string;
  requestObjectEncryptionAlg?: string;
  requestObjectEncryptionEnc?: string;
  tokenEndpointAuthSigningAlg?: string;
  defaultMaxAge?: number;
  requireAuthTime?: boolean;
  defaultAcrValues?: string[];
  initiateLoginUri?: string;
  requestUris?: string[];
  postLogoutRedirectUris?: string[];
  frontchannelLogoutUri?: string;
  frontchannelLogoutSessionRequired?: boolean;
  backchannelLogoutUri?: string;
  backchannelLogoutSessionRequired?: boolean;
  applicationType?: "web" | "native";
  [key: string]: unknown;
};

export type OidcClientMetadataJSON = {
  redirect_uris: string[];
  token_endpoint_auth_method?:
    | "client_secret_basic"
    | "client_secret_post"
    | "client_secret_jwt"
    | "private_key_jwt"
    | "none"
    | string;
  grant_types?: string[];
  response_types?: string[];
  client_name?: string;
  client_uri?: string;
  logo_uri?: string;
  scope?: string;
  contacts?: string[];
  tos_uri?: string;
  policy_uri?: string;
  jwks_uri?: string;
  jwks?: JsonWebKeySet;
  software_id?: string;
  software_version?: string;
  sector_identifier_uri?: string;
  subject_type?: "public" | "pairwise";
  id_token_signed_response_alg?: string;
  id_token_encrypted_response_alg?: string;
  id_token_encrypted_response_enc?: string;
  userinfo_signed_response_alg?: string;
  userinfo_encrypted_response_alg?: string;
  userinfo_encrypted_response_enc?: string;
  request_object_signing_alg?: string;
  request_object_encryption_alg?: string;
  request_object_encryption_enc?: string;
  token_endpoint_auth_signing_alg?: string;
  default_max_age?: number;
  require_auth_time?: boolean;
  default_acr_values?: string[];
  initiate_login_uri?: string;
  request_uris?: string[];
  post_logout_redirect_uris?: string[];
  frontchannel_logout_uri?: string;
  frontchannel_logout_session_required?: boolean;
  backchannel_logout_uri?: string;
  backchannel_logout_session_required?: boolean;
  application_type?: "web" | "native";
  [key: string]: unknown;
};
