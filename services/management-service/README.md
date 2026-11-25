# Management policy persistence

Management creates each Role and Permission as one ofdb Logical Row. Its generated ID is the public policy ID. A peer learns policy from history, not by inserting a copy under a retained ID.

Deletion removes the policy row. After a peer learns the deletion, an older copy cannot restore the row. Grants reference policy IDs and do not confer authority when the referenced policy is absent. Reusing a name creates a new ID and does not transfer any grants.

Initial setup commits the administrator's explicit grants and its setup receipt together on the Installation's identity owner. An exact setup retry does not issue grants again. Peers learn both policy and the receipt through policy history.

This schema requires fresh stores. It does not upgrade earlier stores or exchange their history.

Application policy administration requires a User token for Management. A delegated
Application grant applies only to the token's Application. An explicit Installation
grant for `roles.read`, `roles.write`, `permissions.read`, or `permissions.write`
authorizes that action on Applications, not changes to Installation policy. The
token also needs `management.policy.administer`. IdP adds that scope when a
provisioned operator Client exchanges a User token for the Management audience. The
Installation `roles.read` grant also permits viewing a User's assigned Application
Roles across the Installation. Without that grant, the existing User-role view
remains limited to the token's Application and requires its `users.read` grant.

Fresh default setup provisions the operator Client scope and the four Installation
grants. Existing login Client registrations, setup receipts, and grants are unchanged: restarting or retrying setup does not
grant access to existing administrators or restore removed grants. Existing
Installations need a separate, explicit policy provisioning decision.
