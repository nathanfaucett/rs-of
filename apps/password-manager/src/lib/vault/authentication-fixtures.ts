// Browser-safe test fixtures - no Node.js imports
export const fixturePassword = "synthetic-master-password";
export const fixturePlaintext = JSON.stringify({
	kind: "login",
	id: "12345678-1234-4234-8234-123456789abc",
	name: "Synthetic legacy login",
	username: "synthetic-user",
	secret: "synthetic legacy encrypted content",
	uris: [],
	notes: "",
	favorite: false,
	fields: [],
	history: [],
	createdAt: "2026-01-01T00:00:00.000Z",
	updatedAt: "2026-01-01T00:00:00.000Z",
});
