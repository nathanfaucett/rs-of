import type { ClientRegistration } from "@ofsdk/idp-client";

type CreatedClient = Pick<
  ClientRegistration,
  "clientName" | "profile" | "tokenEndpointAuthMethod"
> & {
  applicationId: string;
  clientId: string;
};

let clients = $state.raw<CreatedClient[]>([]);
let generation = 0;

export const createdClients = {
  get generation() {
    return generation;
  },
  forApplication(applicationId: string) {
    return clients.filter((client) => client.applicationId === applicationId);
  },
  add(
    applicationId: string,
    { clientId, clientName, profile, tokenEndpointAuthMethod }: ClientRegistration
  ) {
    if (clientId)
      clients = [
        ...clients,
        { applicationId, clientId, clientName, profile, tokenEndpointAuthMethod },
      ];
  },
  remove(clientId: string) {
    clients = clients.filter((client) => client.clientId !== clientId);
  },
  clear() {
    generation += 1;
    clients = [];
  },
};
