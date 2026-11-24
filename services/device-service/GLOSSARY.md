# Device glossary

**Device**:
An Installation-level approved endpoint. With permission, it may hold replicas for multiple Users and Applications.

**Device Service**:
The authority for Device records, endpoint identity, approval/revocation, and Device configuration. Management authorizes administration but does not own Device records.

**Device Record ID**:
The identifier of a Device record, distinct from its endpoint public key and any User subject.

**Endpoint Identity**:
The transport identity bound to a Device record by its endpoint public key. It is not an IdP Principal. Proof of key possession is not Device approval or Resource access.

**Device Approval**:
Admission of a Device to an Installation after the required authorization checks. Approval does not grant access to every Resource.

**Device Revocation**:
Withdrawal of Device approval. Revocation prevents later operations when learned; it does not transfer Resource ownership or remotely erase existing copies.

**Resource Selection**:
Desired replication state that maps one Resource to one Device. Selection does not change the Resource's `(User subject, Application)` namespace or replace Resource authorization.
