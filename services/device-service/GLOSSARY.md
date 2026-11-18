# Device glossary

**Device**:
An Installation-level approved endpoint. With permission, it may hold replicas for multiple Users and Applications.

**Device Service**:
The authority for Device records, endpoint identity, approval/revocation, and Device configuration. Management authorizes administration but does not own Device records.

**Endpoint Identity**:
The authenticated transport identity bound to a Device record. Possession of an endpoint key does not itself grant Installation membership or Resource access.

**Device Approval**:
Admission of a Device to an Installation after the required authorization checks. Approval does not grant access to every Resource.

**Device Revocation**:
Withdrawal of Device approval. Revocation prevents later operations when learned; it does not transfer Resource ownership or remotely erase existing copies.

**Resource Selection**:
Desired replication state that maps one Resource to one Device. Selection does not change the Resource's `(User subject, Application)` namespace or replace Resource authorization.
