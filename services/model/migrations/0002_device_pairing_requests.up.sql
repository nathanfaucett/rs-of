-- schema: device
CREATE TABLE IF NOT EXISTS local_device_pairing_requests (device_id TEXT PRIMARY KEY, requester TEXT NOT NULL, public_key TEXT NOT NULL, name TEXT NOT NULL, address TEXT NOT NULL, accepting_public_key TEXT NOT NULL);
