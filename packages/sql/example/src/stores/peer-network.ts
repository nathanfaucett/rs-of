export interface SqlPeerState {
  text: string;
  records: string[];
}

export interface NetworkState<TNetwork, TPeer, TState> {
  network: TNetwork | null;
  peers: TPeer[];
  busy: boolean;
  status: string;
  statusError: boolean;
  editorState: Record<string, TState>;
}

export function createNetworkState<TNetwork, TPeer, TState>(
  initialStatus: string
): NetworkState<TNetwork, TPeer, TState> {
  return {
    network: null,
    peers: [],
    busy: true,
    status: initialStatus,
    statusError: false,
    editorState: {},
  };
}

export function ensureState<TState>(
  editorState: Record<string, TState>,
  peerId: string,
  defaultState: TState
): void {
  if (!(peerId in editorState)) {
    editorState[peerId] = defaultState;
  }
}

export function getDefaultSqlState(): SqlPeerState {
  return { text: "", records: [] };
}
