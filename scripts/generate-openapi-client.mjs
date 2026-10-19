import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import {
  access,
  copyFile,
  cp,
  mkdtemp,
  readFile,
  rename,
  rm,
} from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const service = process.argv[2];
// Current Rust OpenAPI router inventory; group names follow typescript-fetch output.
const expectedApis = {
  idp: {
    DefaultApi:
      "approveForUser authorizeJson authorizeQuery createApplication deleteApplication deleteRegister deleteUser deviceAuth deviceVerify getApplication getRegister getUser health isAllowedForUser jwks listApplications listClientKeys listConsents openapiJson openidConfiguration putRegister register resetPassword revoke revokeClientKeys revokeConsent rotateClientKey sessionsLogout token updateApplication updateUser userinfo version",
    SuperRoutesActiveUserApi: "activeUser",
    SuperRoutesOauth2IntrospectApi: "introspect",
    SuperRoutesReplicaSignersApi:
      "enrollReplicaSigner revokeReplicaSigner rotateReplicaSigner",
  },
  "idp-setup": {
    DefaultApi: "admission cancel create intent join status",
  },
  "idp-independent": {
    DefaultApi: "admission cancel create join ownerPublic recover status",
  },
  management: {
    DefaultApi:
      "assignPermissionToRole assignRoleToUser createPermission createRole deleteDeviceResourceSelection deleteDeviceSelection deletePermission deleteRole device enrollDevice evaluatePermission getDeviceRestriction health listApprovedDeviceEndpoints listDevices listPermissions listRolePermissions listRoles listUserRoles listUserRolesAcrossApplications lookupDeviceEndpoint lookupDeviceIdentity openapiJson pairingAcceptance putDeviceRestriction putDeviceSelection revokeDevice revokePermissionFromRole revokeRoleFromUser revokeSelf setPairingAcceptance transferDevice trustedDevices updateDevice version",
    SuperRoutesReplicationApi: "replicationAdmission selectedResources",
  },
  storage: {
    DefaultApi: "health openapiJson upgradeSocket version",
    ResourcesApi:
      "createDatabase createFileSystem deleteDatabase deleteFileSystem getDatabase getFileSystem listDatabases listFileSystems",
    ResourcesResidencyApi: "getFileSystemResidency putFileSystemResidency",
  },
}[service];
const specUrl = process.env.OPENAPI_SPEC_URL;
const pnpmPath = process.env.npm_execpath;

if (
  !["idp", "idp-setup", "idp-independent", "management", "storage"].includes(
    service,
  )
) {
  throw new Error(
    "Choose the idp, management or storage client generator, or idp-setup/idp-independent",
  );
}
if (!specUrl) {
  throw new Error(
    "Set OPENAPI_SPEC_URL to the live service OpenAPI document URL",
  );
}
if (!pnpmPath) {
  throw new Error(
    "Run this generator through pnpm so its local OpenAPI generator is available",
  );
}

const packageDir = process.cwd();
const sourceDir = path.join(
  packageDir,
  "src",
  ...(service === "storage"
    ? ["generated"]
    : service === "idp-setup"
      ? ["setup"]
      : service === "idp-independent"
        ? ["independent"]
      : []),
);
const temporaryDir = await mkdtemp(
  path.join(path.dirname(packageDir), ".openapi-generation-"),
);
const generatedDir = path.join(temporaryDir, "src");
const backupDir = path.join(packageDir, `.src-backup-${randomUUID()}`);

try {
  const result = spawnSync(
    pnpmPath,
    [
      "exec",
      "openapi-generator-cli",
      "generate",
      "-i",
      specUrl,
      "-g",
      "typescript-fetch",
      "-o",
      generatedDir,
      "--type-mappings=DateTime=Date",
      "--global-property=apiDocs=false,modelDocs=false",
      "--additional-properties=supportsES6=true,typescriptThreePlus=true,withInterfaces=true,useSingleRequestParameter=true,importFileExtension=.js",
    ],
    { cwd: packageDir, stdio: "inherit" },
  );

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(
      `OpenAPI generation failed with exit code ${result.status}`,
    );
  }

  for (const [group, operations] of Object.entries(expectedApis)) {
    let generatedApi;
    try {
      generatedApi = await readFile(
        path.join(generatedDir, "apis", `${group}.ts`),
        "utf8",
      );
    } catch (error) {
      if (error.code !== "ENOENT") {
        throw error;
      }
      throw new Error(
        `The ${service} output is missing required API group ${group}`,
        { cause: error },
      );
    }
    for (const operation of operations.split(" ")) {
      if (
        !new RegExp(`^\\s*async ${operation}\\s*\\(`, "m").test(generatedApi)
      ) {
        throw new Error(
          `The ${service} output is missing required operation ${operation} in ${group}`,
        );
      }
    }
  }

  for (const file of [
    "index.ts",
    "runtime.ts",
    "apis/index.ts",
    "models/index.ts",
  ]) {
    const content = await readFile(path.join(generatedDir, file), "utf8").catch(
      (error) => {
        if (error.code !== "ENOENT") {
          throw error;
        }
        return "";
      },
    );
    if (!content.trim()) {
      throw new Error(
        `The ${service} output has missing or empty required generated file ${file}`,
      );
    }
  }

  // Stage beside the package so its src-only includes cannot exclude temporary output.
  await copyFile(
    path.join(packageDir, "biome.json"),
    path.join(temporaryDir, "biome.json"),
  );
  const formatting = spawnSync(
    pnpmPath,
    [
      "exec",
      "biome",
      "format",
      "--write",
      "--format-with-errors=false",
      "--config-path",
      temporaryDir,
      generatedDir,
    ],
    { cwd: packageDir, stdio: "inherit" },
  );
  if (formatting.error) {
    throw formatting.error;
  }
  if (formatting.status !== 0) {
    throw new Error(
      `OpenAPI formatting failed with exit code ${formatting.status}`,
    );
  }

  if (service === "idp") {
    for (const subtree of ["setup", "independent"]) {
    const setupDir = path.join(sourceDir, subtree);
    const hasSetup = await access(setupDir).then(
      () => true,
      (error) => {
        if (error.code !== "ENOENT") {
          throw error;
        }
        return false;
      },
    );
    if (hasSetup) {
      await cp(setupDir, path.join(generatedDir, subtree), {
        recursive: true,
      });
    }
    }
  }

  let movedExistingSource = false;
  try {
    await access(sourceDir);
    await rename(sourceDir, backupDir);
    movedExistingSource = true;
  } catch (error) {
    if (error.code !== "ENOENT") {
      throw error;
    }
  }

  try {
    await rename(generatedDir, sourceDir);
  } catch (error) {
    if (movedExistingSource) {
      await rename(backupDir, sourceDir);
    }
    throw error;
  }

  if (movedExistingSource) {
    await rm(backupDir, { recursive: true, force: true });
  }
} finally {
  await rm(temporaryDir, { recursive: true, force: true });
}
