#include <stddef.h>
#include <stdio.h>
#include <sys/ioctl.h>
#include "nvtypes.h"
#include "nvkms-api.h"
#include "nvkms-ioctl.h"

#define S(t) printf("sizeof %s = %zu align=%zu\n", #t, sizeof(t), _Alignof(t))
#define O(t, f) printf("offsetof %s.%s = %zu\n", #t, #f, offsetof(t, f))
#define C(c) printf("const %s = %lld\n", #c, (long long)(c))

int main(void) {
    C(NVKMS_IOCTL_IOWR);
    S(struct NvKmsIoctlParams); O(struct NvKmsIoctlParams, cmd); O(struct NvKmsIoctlParams, size); O(struct NvKmsIoctlParams, address);
    C(NVKMS_IOCTL_ALLOC_DEVICE); C(NVKMS_IOCTL_FREE_DEVICE); C(NVKMS_IOCTL_QUERY_DISP);
    C(NVKMS_IOCTL_QUERY_CONNECTOR_STATIC_DATA); C(NVKMS_IOCTL_QUERY_DPY_DYNAMIC_DATA);
    C(NVKMS_IOCTL_SET_DPY_ATTRIBUTE); C(NVKMS_IOCTL_GET_DPY_ATTRIBUTE);
    C(NVKMS_NVIDIA_DRIVER_VERSION_STRING_LENGTH); C(NVKMS_MAX_SUBDEVICES); C(NVKMS_MAX_CONNECTORS_PER_DISP);
    C(NV_KMS_DPY_ATTRIBUTE_DIGITAL_VIBRANCE);
    C(NVKMS_ALLOC_DEVICE_STATUS_SUCCESS); C(NVKMS_ALLOC_DEVICE_STATUS_VERSION_MISMATCH);
    C(NVKMS_ALLOC_DEVICE_STATUS_BAD_REQUEST); C(NVKMS_ALLOC_DEVICE_STATUS_FATAL_ERROR);
    C(NVKMS_ALLOC_DEVICE_STATUS_NO_HARDWARE_AVAILABLE); C(NVKMS_ALLOC_DEVICE_STATUS_CORE_CHANNEL_ALLOC_FAILED);
    S(struct NvKmsDeviceId); S(NVDpyId); S(NvKmsDeviceHandle); S(NvKmsDispHandle); S(NvKmsConnectorHandle);

    S(struct NvKmsAllocDeviceParams); S(struct NvKmsAllocDeviceRequest); S(struct NvKmsAllocDeviceReply);
    O(struct NvKmsAllocDeviceParams, request); O(struct NvKmsAllocDeviceParams, reply);
    O(struct NvKmsAllocDeviceRequest, versionString); O(struct NvKmsAllocDeviceRequest, deviceId); O(struct NvKmsAllocDeviceRequest, no3d);
    O(struct NvKmsAllocDeviceReply, status); O(struct NvKmsAllocDeviceReply, deviceHandle); O(struct NvKmsAllocDeviceReply, numHeads);
    O(struct NvKmsAllocDeviceReply, numDisps); O(struct NvKmsAllocDeviceReply, dispHandles);

    S(struct NvKmsFreeDeviceParams); O(struct NvKmsFreeDeviceParams, request);
    S(struct NvKmsQueryDispParams); S(struct NvKmsQueryDispRequest); S(struct NvKmsQueryDispReply);
    O(struct NvKmsQueryDispParams, reply); O(struct NvKmsQueryDispReply, numConnectors); O(struct NvKmsQueryDispReply, connectorHandles); O(struct NvKmsQueryDispReply, gpuString);

    S(struct NvKmsQueryConnectorStaticDataParams); S(struct NvKmsQueryConnectorStaticDataRequest); S(struct NvKmsQueryConnectorStaticDataReply);
    O(struct NvKmsQueryConnectorStaticDataParams, reply);
    O(struct NvKmsQueryConnectorStaticDataReply, dpyId); O(struct NvKmsQueryConnectorStaticDataReply, type); O(struct NvKmsQueryConnectorStaticDataReply, typeIndex);

    S(struct NvKmsQueryDpyDynamicDataParams); S(struct NvKmsQueryDpyDynamicDataRequest); S(struct NvKmsQueryDpyDynamicDataReply);
    O(struct NvKmsQueryDpyDynamicDataParams, reply); O(struct NvKmsQueryDpyDynamicDataRequest, dpyId);
    O(struct NvKmsQueryDpyDynamicDataReply, name); O(struct NvKmsQueryDpyDynamicDataReply, connected);

    S(struct NvKmsSetDpyAttributeParams); S(struct NvKmsSetDpyAttributeRequest); S(struct NvKmsSetDpyAttributeReply);
    O(struct NvKmsSetDpyAttributeParams, reply);
    O(struct NvKmsSetDpyAttributeRequest, dpyId); O(struct NvKmsSetDpyAttributeRequest, attribute); O(struct NvKmsSetDpyAttributeRequest, value);
    S(struct NvKmsGetDpyAttributeParams); S(struct NvKmsGetDpyAttributeRequest); S(struct NvKmsGetDpyAttributeReply);
    O(struct NvKmsGetDpyAttributeParams, reply); O(struct NvKmsGetDpyAttributeReply, value);
    return 0;
}
