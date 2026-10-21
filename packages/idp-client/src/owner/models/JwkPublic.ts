// Generated from the canonical allOf/oneOf composition.
import {
    type JwkPublicParameters,
    JwkPublicParametersFromJSON,
    JwkPublicParametersToJSON,
} from "./JwkPublicParameters.js";
import {
    type JwsAlgorithm,
    JwsAlgorithmFromJSON,
    JwsAlgorithmToJSON,
} from "./JwsAlgorithm.js";
import { type KeyUse, KeyUseFromJSON, KeyUseToJSON } from "./KeyUse.js";
export type JwkPublic = JwkPublicParameters & {
    alg: JwsAlgorithm;
    kid: string;
    use: KeyUse;
};
export function instanceOfJwkPublic(value: object): value is JwkPublic {
    return ["use", "kid", "alg"].every((key) => key in value);
}
export function JwkPublicFromJSON(json: any): JwkPublic {
    return JwkPublicFromJSONTyped(json, false);
}
export function JwkPublicFromJSONTyped(
    json: any,
    ignoreDiscriminator: boolean,
): JwkPublic {
    if (json == null) return json;
    const parameters = JwkPublicParametersFromJSON(json);
    if (!parameters || !Object.keys(parameters).length)
        throw new Error("Invalid JwkPublicParameters");
    return {
        ...parameters,
        alg: JwsAlgorithmFromJSON(json["alg"]),
        kid: json["kid"],
        use: KeyUseFromJSON(json["use"]),
    };
}
export function JwkPublicToJSON(value: any): any {
    return JwkPublicToJSONTyped(value);
}
export function JwkPublicToJSONTyped(
    value?: JwkPublic | null,
    ignoreDiscriminator = false,
): any {
    if (value == null) return value;
    return {
        ...JwkPublicParametersToJSON(value),
        alg: JwsAlgorithmToJSON(value["alg"]),
        kid: value["kid"],
        use: KeyUseToJSON(value["use"]),
    };
}
