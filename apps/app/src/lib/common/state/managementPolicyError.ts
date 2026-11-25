import { ResponseError as IdpResponseError } from "@ofsdk/idp-client";
import { ResponseError } from "@ofsdk/management-client";

export function managementPolicyStatus(cause: unknown) {
  return cause instanceof ResponseError || cause instanceof IdpResponseError
    ? cause.response.status
    : undefined;
}

export function managementPolicyError(cause: unknown, action: string) {
  switch (managementPolicyStatus(cause)) {
    case 401:
      return "Sign in again to continue.";
    case 403:
      return `You do not have permission to ${action}. Ask an administrator for access.`;
    case 400:
    case 422:
      return `Cannot ${action}. Check the values and try again.`;
    case 404:
      return `Cannot ${action}. The record no longer exists. Reload the page.`;
    case 409:
      return `Cannot ${action}. The record conflicts with existing policy.`;
    default:
      return `Cannot ${action} now. Try again.`;
  }
}
