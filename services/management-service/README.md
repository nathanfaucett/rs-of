# Management policy persistence

Management creates each Role and Permission as one ofdb Logical Row. Its generated ID is the public policy ID. A peer learns policy from history, not by inserting a copy under a retained ID.

Deletion removes the policy row. After a peer learns the deletion, an older copy cannot restore the row. Grants reference policy IDs and do not confer authority when the referenced policy is absent. Reusing a name creates a new ID and does not transfer any grants.

Initial setup commits the administrator's explicit grants and its setup receipt together on the Installation's identity owner. Setup retries do not issue grants again. New Installations include `idp.users.create` in the initial administrator role. Existing Installations keep their policy. A retry accepts the previous default setup receipt without adding the permission. Peers learn both policy and the receipt through policy history.

This schema requires fresh stores. It does not upgrade earlier stores or exchange their history.
