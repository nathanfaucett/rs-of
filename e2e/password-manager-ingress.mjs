// Ticket 04 owns the real descriptor and CORS. This fixture forwards OAuth and Storage to their owners.
export function passwordManagerIngress(req, res, origin, descriptor) {
  if (req.headers.origin !== origin) return false;
  if (descriptor && req.url === "/setup/node" && req.method === "GET") {
    res.writeHead(200, {
      "access-control-allow-origin": origin,
      "cache-control": "no-store",
      "content-type": "application/json",
    });
    res.end(JSON.stringify(descriptor));
    return true;
  }
  if (
    req.url.startsWith("/idp/oauth2/") ||
    req.url === "/idp/userinfo" ||
    req.url === "/idp/.well-known/openid-configuration" ||
    req.url.startsWith("/storage/databases")
  ) {
    if (req.method === "OPTIONS") {
      res.writeHead(204, {
        "access-control-allow-origin": origin,
        "access-control-allow-methods": "GET, POST",
        "access-control-allow-headers": "authorization, content-type",
      });
      res.end();
      return true;
    }
    res.setHeader("access-control-allow-origin", origin);
  }
  return false;
}
