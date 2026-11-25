import type { DeviceEnrollment, DeviceInfo, DeviceState } from "@ofsdk/device-client";
import { deviceApi } from "./deviceClient.svelte";

export type { DeviceEnrollment, DeviceInfo, DeviceState };

export function listDevices(): Promise<DeviceInfo[]> {
  return deviceApi.listDevices();
}

export function renameDevice(id: string, name: string): Promise<DeviceInfo> {
  return deviceApi.updateDevice({ id, updateDeviceRequest: { name } });
}

export function revokeDevice(id: string): Promise<void> {
  return deviceApi.revokeDevice({ id });
}

export async function enrollDevice(
  address: string,
  name: string,
  publicKey: string
): Promise<DeviceEnrollment> {
  const enrollment = await deviceApi.enrollDevice({
    deviceEnrollmentRequest: { address, name, publicKey },
  });
  return enrollment;
}

export async function approveDeviceEnrollment(enrollmentId: string): Promise<DeviceEnrollment> {
  return deviceApi.approveEnrollment({ deviceId: enrollmentId });
}

export function listPendingEnrollments(): Promise<DeviceEnrollment[]> {
  return deviceApi.listPendingEnrollments();
}
